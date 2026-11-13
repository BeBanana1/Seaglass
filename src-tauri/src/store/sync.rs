//! Échange de l'historique d'écoute avec un autre appareil (#430).
//!
//! Une écoute est identifiée par son titre et son horodatage (ms) : la fusion ne garde
//! qu'une ligne par paire, quel que soit l'appareil qui l'envoie, et on peut renvoyer un lot
//! sans risque. `plays.device` garde l'appareil d'origine d'une écoute reçue (`NULL` =
//! écoutée ici) : on ne renvoie pas à un appareil ses propres écoutes.
//!
//! Les curseurs sont des `plays.id` (AUTOINCREMENT : jamais réutilisés, même après un
//! effacement, qui reste donc local). C'est toujours le téléphone qui les tient
//! ([`SyncState`]) ; le PC répond sans rien retenir, ce qui permet plusieurs téléphones.

use std::collections::BTreeSet;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::plays::prune;
use super::{Result, Store, StoreError};
use crate::player::source::PlaySource;
use crate::tidal::models::{AlbumSummary, Track};

/// Écoutes par lot (un appel HTTP) : environ 1 Mo avec les fiches des titres et albums.
pub const BATCH: usize = 1_000;
/// Un nom d'artiste reçu est coupé au-delà.
const MAX_ARTIST_CHARS: usize = 300;
/// Une écoute datée de plus d'un jour dans le futur est refusée (horloge folle).
const FUTURE_MS: i64 = 24 * 3600 * 1000;
const KEY_DEVICE: &str = "device.id";
const KEY_STATE: &str = "history.sync";

/// Une écoute telle qu'elle voyage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPlay {
    /// Appareil où elle a été écoutée.
    pub device: String,
    pub track_id: u64,
    #[serde(default)]
    pub album_id: Option<u64>,
    #[serde(default)]
    pub source: Option<PlaySource>,
    /// Millisecondes depuis l'epoch Unix.
    pub played_at: i64,
    #[serde(default)]
    pub artist_id: Option<u64>,
    #[serde(default)]
    pub artist: Option<String>,
    /// Secondes.
    #[serde(default)]
    pub duration: Option<i64>,
}

/// Lot d'écoutes avec les fiches de leurs titres et albums (dernière copie connue).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayBatch {
    pub plays: Vec<SyncPlay>,
    #[serde(default)]
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub albums: Vec<AlbumSummary>,
}

impl PlayBatch {
    /// Lot reçu d'un autre appareil, vérifié : `None` s'il est trop gros ; les écoutes
    /// invalides (appareil, date, durée) sont écartées, les noms coupés, la source nettoyée.
    pub fn received(self, now_ms: i64, is_uuid: impl Fn(&str) -> bool) -> Option<Self> {
        let max = self.plays.len();
        if max > BATCH || self.tracks.len() > max || self.albums.len() > max {
            return None;
        }
        let plays = self
            .plays
            .into_iter()
            .filter(|p| {
                is_uuid(&p.device)
                    && (0..=now_ms + FUTURE_MS).contains(&p.played_at)
                    && p.duration.is_none_or(|d| (0..=7 * 24 * 3600).contains(&d))
                    && p.track_id <= i64::MAX as u64
                    && p.album_id.is_none_or(|id| id <= i64::MAX as u64)
                    && p.artist_id.is_none_or(|id| id <= i64::MAX as u64)
            })
            .map(|p| SyncPlay {
                artist: p.artist.map(|a| a.chars().take(MAX_ARTIST_CHARS).collect()),
                source: p.source.and_then(|s| s.sanitize(&is_uuid)),
                ..p
            })
            .collect();
        Some(Self {
            plays,
            tracks: self.tracks,
            albums: self.albums,
        })
    }
}

/// Écoutes à envoyer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outgoing<'a> {
    /// Celles écoutées ici seulement (le téléphone vers le PC).
    Own,
    /// Toutes, sauf celles venues de cet appareil (le PC vers un téléphone).
    Except(&'a str),
}

/// Lot à envoyer et curseur à retenir.
#[derive(Debug, Clone, PartialEq)]
pub struct Outbox {
    pub batch: PlayBatch,
    /// Dernier `plays.id` parcouru : prochain `after`.
    pub cursor: i64,
    /// Il en reste après ce lot.
    pub more: bool,
}

/// Où en est la synchro du téléphone avec son ordinateur lié.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncState {
    /// Identifiant de l'ordinateur : s'il change (réinstallé), tout est repris de zéro.
    pub peer: Option<String>,
    /// Dernier id de l'ordinateur reçu.
    pub pulled: i64,
    /// Dernier id d'ici envoyé.
    pub pushed: i64,
    /// Dernière synchro réussie (ms).
    #[serde(default)]
    pub synced_at: Option<i64>,
}

/// Identifiant aléatoire au format UUID (version 4).
fn new_device_id() -> String {
    let bytes: [u8; 16] = rand::random();
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-4{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[13..16],
        &hex[16..20],
        &hex[20..32]
    )
}

impl Store {
    /// Identifiant de cet appareil, tiré une fois.
    pub fn device_id(&self) -> Result<String> {
        if let Some(id) = self.get_setting::<String>(KEY_DEVICE)? {
            return Ok(id);
        }
        let id = new_device_id();
        self.set_setting(KEY_DEVICE, &id)?;
        Ok(id)
    }

    pub fn sync_state(&self) -> Result<SyncState> {
        Ok(self.get_setting(KEY_STATE)?.unwrap_or_default())
    }

    pub fn set_sync_state(&self, state: &SyncState) -> Result<()> {
        self.set_setting(KEY_STATE, state)
    }

    /// Les `limit` écoutes suivant `after` (par id), avec leurs fiches.
    pub fn outgoing_plays(&self, after: i64, filter: Outgoing, limit: usize) -> Result<Outbox> {
        let own = self.device_id()?;
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let conn = &db.conn;
        let (sql, except) = match filter {
            Outgoing::Own => ("p.device IS NULL", ""),
            Outgoing::Except(device) => ("(p.device IS NULL OR p.device <> ?3)", device),
        };
        let sql = format!(
            "SELECT p.id, p.device, p.track_id, p.album_id, p.source, p.played_at,
                    p.artist_id, p.artist, p.duration
             FROM plays p WHERE p.id > ?1 AND {sql} ORDER BY p.id LIMIT ?2"
        );
        let mut stmt = conn.prepare(&sql)?;
        let map = |r: &rusqlite::Row| {
            Ok((
                r.get::<_, i64>(0)?,
                SyncPlay {
                    device: r
                        .get::<_, Option<String>>(1)?
                        .unwrap_or_else(|| own.clone()),
                    track_id: r.get::<_, i64>(2)? as u64,
                    album_id: r.get::<_, Option<i64>>(3)?.map(|id| id as u64),
                    source: r
                        .get::<_, Option<String>>(4)?
                        .and_then(|s| serde_json::from_str(&s).ok()),
                    played_at: r.get(5)?,
                    artist_id: r.get::<_, Option<i64>>(6)?.map(|id| id as u64),
                    artist: r.get(7)?,
                    duration: r.get(8)?,
                },
            ))
        };
        let rows: Vec<(i64, SyncPlay)> = match filter {
            Outgoing::Own => stmt
                .query_map(params![after, limit as i64], map)?
                .collect::<rusqlite::Result<_>>()?,
            Outgoing::Except(_) => stmt
                .query_map(params![after, limit as i64, except], map)?
                .collect::<rusqlite::Result<_>>()?,
        };
        let more = rows.len() == limit;
        // Rien à envoyer : on saute aussi les lignes écartées par le filtre.
        let cursor = match rows.last() {
            Some((id, _)) => *id,
            None => conn
                .query_row("SELECT MAX(id) FROM plays", [], |r| {
                    r.get::<_, Option<i64>>(0)
                })?
                .unwrap_or(0)
                .max(after),
        };
        let plays: Vec<SyncPlay> = rows.into_iter().map(|(_, p)| p).collect();
        let tracks = copies(conn, "played_tracks", plays.iter().map(|p| p.track_id))?;
        let albums = copies(
            conn,
            "played_albums",
            plays.iter().filter_map(|p| p.album_id),
        )?;
        Ok(Outbox {
            batch: PlayBatch {
                plays,
                tracks,
                albums,
            },
            cursor,
            more,
        })
    }

    /// Ajoute les écoutes reçues qui manquent ici ; renvoie leur nombre. Les fiches ne
    /// remplacent pas celles d'ici. Les écoutes venues de `own` (cet appareil) sont ignorées.
    pub fn merge_plays(&self, batch: &PlayBatch, own: &str) -> Result<usize> {
        let mut db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let tx = db.conn.transaction()?;
        let mut added = 0;
        for play in batch.plays.iter().filter(|p| p.device != own) {
            let source = play
                .source
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?;
            added += tx.execute(
                "INSERT INTO plays
                     (track_id, album_id, source, played_at, artist_id, artist, duration, device)
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8
                 WHERE NOT EXISTS
                     (SELECT 1 FROM plays WHERE track_id = ?1 AND played_at = ?4)",
                params![
                    play.track_id as i64,
                    play.album_id.map(|id| id as i64),
                    source,
                    play.played_at,
                    play.artist_id.map(|id| id as i64),
                    play.artist,
                    play.duration,
                    play.device,
                ],
            )?;
        }
        if added > 0 {
            // Une fiche d'ici est au moins aussi récente : gardée.
            for track in &batch.tracks {
                tx.execute(
                    "INSERT OR IGNORE INTO played_tracks (id, json) VALUES (?1, ?2)",
                    params![track.id.0 as i64, serde_json::to_string(track)?],
                )?;
            }
            for album in &batch.albums {
                tx.execute(
                    "INSERT OR IGNORE INTO played_albums (id, json) VALUES (?1, ?2)",
                    params![album.id as i64, serde_json::to_string(album)?],
                )?;
            }
            prune(&tx)?;
        }
        tx.commit()?;
        Ok(added)
    }
}

/// Copies lisibles de `table` (`played_tracks` ou `played_albums`) pour ces ids.
fn copies<T: serde::de::DeserializeOwned>(
    conn: &Connection,
    table: &str,
    ids: impl Iterator<Item = u64>,
) -> Result<Vec<T>> {
    let ids: BTreeSet<u64> = ids.collect();
    let mut stmt = conn.prepare(&format!("SELECT json FROM {table} WHERE id = ?1"))?;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let json: Option<String> = stmt.query_row([id as i64], |r| r.get(0)).optional()?;
        if let Some(copy) = json.and_then(|j| serde_json::from_str(&j).ok()) {
            out.push(copy);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::plays::tests::play;
    use super::*;
    use crate::commands::settings::pins::is_uuid;

    const PHONE: &str = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";

    fn sent(store: &Store, after: i64, filter: Outgoing) -> Outbox {
        store.outgoing_plays(after, filter, BATCH).unwrap()
    }

    fn played(store: &Store) -> Vec<(u64, i64, Option<String>)> {
        let db = store.db.lock().unwrap();
        db.conn
            .prepare("SELECT track_id, played_at, device FROM plays ORDER BY played_at")
            .unwrap()
            .query_map([], |r| {
                Ok((r.get::<_, i64>(0)? as u64, r.get(1)?, r.get(2)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    fn device_ids_are_stable_uuids() {
        let store = Store::in_memory().unwrap();
        let id = store.device_id().unwrap();
        assert!(is_uuid(&id), "{id}");
        assert_eq!(store.device_id().unwrap(), id);
        assert_ne!(new_device_id(), new_device_id());
    }

    #[test]
    fn two_devices_converge_without_duplicates() {
        let pc = Store::in_memory().unwrap();
        let phone = Store::in_memory().unwrap();
        play(&pc, 1, 10, 100);
        play(&pc, 2, 20, 300);
        play(&phone, 3, 30, 200);
        let pc_id = pc.device_id().unwrap();
        let phone_id = phone.device_id().unwrap();

        // Le téléphone envoie les siennes, le PC répond avec les autres.
        let push = sent(&phone, 0, Outgoing::Own);
        assert_eq!(push.batch.plays.len(), 1);
        assert_eq!(push.batch.plays[0].device, phone_id);
        assert_eq!(push.batch.tracks.len(), 1);
        assert_eq!(pc.merge_plays(&push.batch, &pc_id).unwrap(), 1);
        let pull = sent(&pc, 0, Outgoing::Except(&phone_id));
        assert_eq!(
            pull.batch.plays.len(),
            2,
            "pas d'écho de ses propres écoutes"
        );
        assert_eq!(phone.merge_plays(&pull.batch, &phone_id).unwrap(), 2);

        // Renvoyer les mêmes lots n'ajoute rien.
        assert_eq!(pc.merge_plays(&push.batch, &pc_id).unwrap(), 0);
        assert_eq!(phone.merge_plays(&pull.batch, &phone_id).unwrap(), 0);
        assert_eq!(played(&pc).len(), 3);
        assert_eq!(
            played(&pc).iter().map(|p| p.0).collect::<Vec<_>>(),
            played(&phone).iter().map(|p| p.0).collect::<Vec<_>>()
        );
        // L'origine est gardée ; les écoutes d'ici restent à `NULL`.
        assert_eq!(played(&phone)[0].2.as_deref(), Some(pc_id.as_str()));
        assert_eq!(played(&phone)[1].2, None);
        // Les albums reçus servent aux classements, dans l'ordre des dates.
        let recent: Vec<u64> = phone
            .recent_albums(10)
            .unwrap()
            .into_iter()
            .map(|a| a.id)
            .collect();
        assert_eq!(recent, [20, 30, 10]);
    }

    #[test]
    fn cursors_page_and_skip_filtered_rows() {
        let pc = Store::in_memory().unwrap();
        for i in 0..5 {
            play(&pc, i, 1, i as i64);
        }
        let first = pc.outgoing_plays(0, Outgoing::Except(PHONE), 2).unwrap();
        assert_eq!(first.batch.plays.len(), 2);
        assert!(first.more);
        let rest = pc
            .outgoing_plays(first.cursor, Outgoing::Except(PHONE), 10)
            .unwrap();
        assert_eq!(rest.batch.plays.len(), 3);
        assert!(!rest.more);
        // Écoutes venues du téléphone seulement : rien à renvoyer, curseur au bout.
        let echo = PlayBatch {
            plays: vec![SyncPlay {
                device: PHONE.into(),
                track_id: 99,
                album_id: None,
                source: None,
                played_at: 1_000,
                artist_id: None,
                artist: None,
                duration: None,
            }],
            ..PlayBatch::default()
        };
        assert_eq!(pc.merge_plays(&echo, &pc.device_id().unwrap()).unwrap(), 1);
        let after = pc
            .outgoing_plays(rest.cursor, Outgoing::Except(PHONE), 10)
            .unwrap();
        assert!(after.batch.plays.is_empty());
        assert!(after.cursor > rest.cursor);
        // Et le PC n'envoie jamais au téléphone ce qu'il a reçu de lui comme « à lui ».
        assert!(
            sent(&pc, 0, Outgoing::Own)
                .batch
                .plays
                .iter()
                .all(|p| p.track_id != 99)
        );
    }

    #[test]
    fn clearing_stays_local() {
        let pc = Store::in_memory().unwrap();
        play(&pc, 1, 1, 1);
        let before = sent(&pc, 0, Outgoing::Except(PHONE));
        pc.clear_plays().unwrap();
        play(&pc, 2, 1, 2);
        // Les ids ne reviennent pas en arrière : le curseur reste valable.
        let after = sent(&pc, before.cursor, Outgoing::Except(PHONE));
        assert_eq!(after.batch.plays.len(), 1);
        assert_eq!(after.batch.plays[0].track_id, 2);
    }

    #[test]
    fn received_batches_are_checked() {
        let good = SyncPlay {
            device: PHONE.into(),
            track_id: 1,
            album_id: Some(2),
            source: Some(PlaySource::Search {
                query: "  x  ".into(),
            }),
            played_at: 1_000,
            artist_id: Some(3),
            artist: Some("a".repeat(1_000)),
            duration: Some(200),
        };
        let batch = PlayBatch {
            plays: vec![
                good.clone(),
                SyncPlay {
                    device: "pas un uuid".into(),
                    ..good.clone()
                },
                SyncPlay {
                    played_at: i64::MAX,
                    ..good.clone()
                },
                SyncPlay {
                    duration: Some(-1),
                    ..good.clone()
                },
            ],
            ..PlayBatch::default()
        };
        let checked = batch.received(10_000, is_uuid).unwrap();
        assert_eq!(checked.plays.len(), 1);
        assert_eq!(
            checked.plays[0].artist.as_ref().unwrap().chars().count(),
            300
        );
        assert_eq!(
            checked.plays[0].source,
            Some(PlaySource::Search { query: "x".into() })
        );
        let huge = PlayBatch {
            plays: vec![good; BATCH + 1],
            ..PlayBatch::default()
        };
        assert!(huge.received(10_000, is_uuid).is_none());
    }

    #[test]
    fn sync_state_round_trips() {
        let store = Store::in_memory().unwrap();
        assert_eq!(store.sync_state().unwrap(), SyncState::default());
        let state = SyncState {
            peer: Some(PHONE.into()),
            pulled: 4,
            pushed: 7,
            synced_at: Some(9),
        };
        store.set_sync_state(&state).unwrap();
        assert_eq!(store.sync_state().unwrap(), state);
    }
}
