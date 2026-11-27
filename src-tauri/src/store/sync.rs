//! Exchange of the listening history with another device (#430).
//!
//! A play is identified by its track and its timestamp (ms): the merge keeps one row per
//! pair, whatever device sends it, and a batch can be sent again safely. `plays.device`
//! keeps the source device of a received play (`NULL` = played here): a device is never
//! sent its own plays back.
//!
//! The cursors are `plays.id` values (AUTOINCREMENT: never reused, even after a deletion,
//! which therefore stays local). The phone always holds them ([`SyncState`]); the PC
//! answers without keeping anything, which allows several phones.

use std::collections::BTreeSet;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::formats::{self, PlayFormat};
use super::plays::prune;
use super::{Result, Store, StoreError};
use crate::player::source::PlaySource;
use crate::tidal::models::{AlbumSummary, Track};

/// Plays per batch (one HTTP call): about 1 MB with the track and album records.
pub const BATCH: usize = 1_000;
/// A received artist name is cut beyond this.
const MAX_ARTIST_CHARS: usize = 300;
/// A play dated more than a day in the future is refused (a broken clock).
const FUTURE_MS: i64 = 24 * 3600 * 1000;
const KEY_DEVICE: &str = "device.id";
const KEY_STATE: &str = "history.sync";

/// A play as it travels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPlay {
    /// Device it was played on.
    pub device: String,
    pub track_id: u64,
    #[serde(default)]
    pub album_id: Option<u64>,
    #[serde(default)]
    pub source: Option<PlaySource>,
    /// Milliseconds since the Unix epoch.
    pub played_at: i64,
    #[serde(default)]
    pub artist_id: Option<u64>,
    #[serde(default)]
    pub artist: Option<String>,
    /// Secondes.
    #[serde(default)]
    pub duration: Option<i64>,
    /// Format played (#524); absent from older versions, which ignore it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<PlayFormat>,
}

/// A batch of plays with the records of their tracks and albums (last known copy).
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
    /// A batch received from another device, checked: `None` if it is too large; invalid
    /// plays (device, date, duration) are dropped, names cut, the source cleaned.
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
                format: p.format.and_then(PlayFormat::sanitize),
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

/// Plays to send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outgoing<'a> {
    /// Only those played here (the phone to the PC).
    Own,
    /// All, except those that came from this device (the PC to a phone).
    Except(&'a str),
}

/// Batch to send and cursor to keep.
#[derive(Debug, Clone, PartialEq)]
pub struct Outbox {
    pub batch: PlayBatch,
    /// Last `plays.id` covered: next `after`.
    pub cursor: i64,
    /// More are left after this batch.
    pub more: bool,
}

/// How far the phone's sync with its linked computer has got.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncState {
    /// The computer's id: if it changes (reinstalled), everything starts again from zero.
    pub peer: Option<String>,
    /// Last id received from the computer.
    pub pulled: i64,
    /// Last id from here that was sent.
    pub pushed: i64,
    /// Last successful sync (ms).
    #[serde(default)]
    pub synced_at: Option<i64>,
}

/// A random id in UUID format (version 4).
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
    /// This device's id, drawn once.
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

    /// The `limit` plays after `after` (by id), with their records.
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
                    p.artist_id, p.artist, p.duration, {}
             FROM plays p WHERE p.id > ?1 AND {sql} ORDER BY p.id LIMIT ?2",
            formats::COLUMNS
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
                    format: formats::read(r, 9)?,
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
        // Nothing to send: the rows dropped by the filter are skipped too.
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

    /// Adds the received plays missing here; returns how many. The records do not replace
    /// those from here. Plays that came from `own` (this device) are ignored.
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
            let (quality, codec, bit_depth, sample_rate, bit_perfect) =
                formats::values(play.format.as_ref());
            added += tx.execute(
                &format!(
                    "INSERT INTO plays
                         (track_id, album_id, source, played_at, artist_id, artist, duration,
                          device, {})
                     SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13
                     WHERE NOT EXISTS
                         (SELECT 1 FROM plays WHERE track_id = ?1 AND played_at = ?4)",
                    formats::COLUMNS
                ),
                params![
                    play.track_id as i64,
                    play.album_id.map(|id| id as i64),
                    source,
                    play.played_at,
                    play.artist_id.map(|id| id as i64),
                    play.artist,
                    play.duration,
                    play.device,
                    quality,
                    codec,
                    bit_depth,
                    sample_rate,
                    bit_perfect,
                ],
            )?;
        }
        if added > 0 {
            // A record from here is at least as recent: kept.
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

/// Readable copies from `table` (`played_tracks` or `played_albums`) for these ids.
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

        // The phone sends its own, the PC answers with the others.
        let push = sent(&phone, 0, Outgoing::Own);
        assert_eq!(push.batch.plays.len(), 1);
        assert_eq!(push.batch.plays[0].device, phone_id);
        assert_eq!(push.batch.tracks.len(), 1);
        assert_eq!(pc.merge_plays(&push.batch, &pc_id).unwrap(), 1);
        let pull = sent(&pc, 0, Outgoing::Except(&phone_id));
        assert_eq!(pull.batch.plays.len(), 2, "no echo of its own plays");
        assert_eq!(phone.merge_plays(&pull.batch, &phone_id).unwrap(), 2);

        // Sending the same batches again adds nothing.
        assert_eq!(pc.merge_plays(&push.batch, &pc_id).unwrap(), 0);
        assert_eq!(phone.merge_plays(&pull.batch, &phone_id).unwrap(), 0);
        assert_eq!(played(&pc).len(), 3);
        assert_eq!(
            played(&pc).iter().map(|p| p.0).collect::<Vec<_>>(),
            played(&phone).iter().map(|p| p.0).collect::<Vec<_>>()
        );
        // The source is kept; plays from here stay at `NULL`.
        assert_eq!(played(&phone)[0].2.as_deref(), Some(pc_id.as_str()));
        assert_eq!(played(&phone)[1].2, None);
        // The received albums count in the rankings, in date order.
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
        // Plays that came from the phone only: nothing to send back, cursor at the end.
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
                format: None,
            }],
            ..PlayBatch::default()
        };
        assert_eq!(pc.merge_plays(&echo, &pc.device_id().unwrap()).unwrap(), 1);
        let after = pc
            .outgoing_plays(rest.cursor, Outgoing::Except(PHONE), 10)
            .unwrap();
        assert!(after.batch.plays.is_empty());
        assert!(after.cursor > rest.cursor);
        // And the PC never sends the phone what it received from it as "its own".
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
        // Ids never go backwards: the cursor stays valid.
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
            format: Some(formats::tests::format("LOSSLESS", 16, 44100, false)),
        };
        let batch = PlayBatch {
            plays: vec![
                good.clone(),
                SyncPlay {
                    device: "not a uuid".into(),
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
                // Unknown quality (newer version): the listen is kept without its format.
                SyncPlay {
                    track_id: 7,
                    format: Some(PlayFormat {
                        quality: "DOLBY_ATMOS".into(),
                        ..formats::tests::format("LOSSLESS", 16, 44100, false)
                    }),
                    ..good.clone()
                },
            ],
            ..PlayBatch::default()
        };
        let checked = batch.received(10_000, is_uuid).unwrap();
        assert_eq!(checked.plays.len(), 2);
        assert!(checked.plays[0].format.is_some());
        assert_eq!(checked.plays[1].format, None);
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

    #[test]
    fn formats_travel_with_their_listens() {
        let pc = Store::in_memory().unwrap();
        let phone = Store::in_memory().unwrap();
        let hires = formats::tests::format("HI_RES_LOSSLESS", 24, 96000, true);
        phone
            .add_play(&super::super::plays::Play {
                track: &super::super::plays::tests::track(1, 10, 110),
                album: &super::super::plays::tests::album(10),
                source: None,
                played_at: 100,
                format: Some(&hires),
            })
            .unwrap();
        play(&phone, 2, 10, 200);
        let push = sent(&phone, 0, Outgoing::Own);
        assert_eq!(push.batch.plays[0].format, Some(hires));
        assert_eq!(push.batch.plays[1].format, None);
        let json = serde_json::to_string(&push.batch.plays[1]).unwrap();
        assert!(!json.contains("format"), "{json}");
        pc.merge_plays(&push.batch, &pc.device_id().unwrap())
            .unwrap();
        let totals = pc.audio_totals(super::super::Span::since(0)).unwrap();
        assert_eq!((totals.known, totals.hires, totals.bit_perfect), (1, 1, 1));
    }
}
