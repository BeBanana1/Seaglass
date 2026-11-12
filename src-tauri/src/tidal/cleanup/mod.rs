//! « Nettoyer » une playlist de l'utilisateur ou ses titres likés (#382) : doublons et titres
//! retirés du catalogue, avec un remplaçant lisible quand on en trouve un.
//!
//! Vérifié le 2026-11-12 (`cargo test live_cleanup_probe -- --ignored --nocapture`) :
//! `playlists/{uuid}/tracks` renvoie **tous** les titres, indisponibles compris
//! (`streamReady` et `allowStreaming` à `false`) : les positions restent justes.
//! `favorites/tracks` en cache certains (433 reçus pour 460 annoncés), et ceux-là répondent
//! 404 à `tracks/{id}` : rien à proposer pour eux. Remplaçant : `tracks?isrc=`, sinon une
//! recherche « artiste titre » (règles dans `rules.rs`).

pub mod ops;
pub mod rules;

use std::collections::HashMap;

use super::api::TidalClient;
use super::catalog::{MAX_TRACKS, PAGE_SIZE};
use super::models::{
    CleanupItem, CleanupReport, LikeTarget, ListOp, ListOpsResult, Track, TrackId,
};
use super::paging::try_join_bounded;
use super::raw::{RawFavorite, RawPage, RawTrack};
use super::{Result, TidalError};
use rules::Entry;

/// Recherches de remplaçants en vol au plus.
const LOOKUPS_IN_FLIGHT: usize = 4;
/// Résultats lus par recherche de remplaçant.
const SEARCH_LIMIT: u32 = 20;
/// Favoris parcourus au plus (comme la bibliothèque).
const LIKES_MAX: usize = super::catalog::LIBRARY_MAX;

impl TidalClient {
    /// Changements proposés pour la playlist `uuid`, relue en entier (sans la copie gardée).
    pub async fn playlist_cleanup(&self, uuid: &str) -> Result<CleanupReport> {
        let path = format!("playlists/{uuid}/tracks");
        let raw = self
            .get_all::<RawTrack>(&path, &[], PAGE_SIZE, MAX_TRACKS)
            .await?;
        Ok(self.report(entries(raw)).await)
    }

    /// Changements proposés pour les titres likés (du plus récent au plus ancien).
    pub async fn liked_cleanup(&self) -> Result<CleanupReport> {
        let user = self.user_id().await?;
        let path = format!("users/{user}/favorites/tracks");
        let raw = self
            .get_all::<RawFavorite<RawTrack>>(&path, &[], PAGE_SIZE, LIKES_MAX)
            .await?;
        Ok(self
            .report(entries(raw.into_iter().map(|f| f.item).collect()))
            .await)
    }

    async fn report(&self, entries: Vec<Entry>) -> CleanupReport {
        let scanned = entries.len();
        let (mut items, to_replace) = rules::duplicates(&entries);
        // Première position lisible de chaque titre : un remplaçant déjà dans la liste fait
        // de l'indisponible un simple doublon.
        let mut present: HashMap<TrackId, &Entry> = HashMap::new();
        for e in entries.iter().filter(|e| e.available) {
            present.entry(e.track.id).or_insert(e);
        }
        let mut lookups = Vec::with_capacity(to_replace.len());
        for &i in &to_replace {
            lookups.push(self.replacement_ok(&entries[i].track));
        }
        let found = try_join_bounded(lookups, LOOKUPS_IN_FLIGHT)
            .await
            .unwrap_or_default();
        for (&i, replacement) in to_replace.iter().zip(found) {
            let entry = &entries[i];
            let kept = replacement.as_ref().and_then(|r| present.get(&r.id));
            items.push(match kept {
                Some(kept) => CleanupItem::Duplicate {
                    index: entry.index,
                    track: entry.track.clone(),
                    kept_index: kept.index,
                    kept: Box::new(kept.track.clone()),
                    same_track: false,
                },
                None => CleanupItem::Unavailable {
                    index: entry.index,
                    track: entry.track.clone(),
                    replacement: replacement.map(Box::new),
                },
            });
        }
        items.sort_by_key(|item| match item {
            CleanupItem::Duplicate { index, .. } | CleanupItem::Unavailable { index, .. } => *index,
        });
        CleanupReport {
            items: items.into_iter().map(Into::into).collect(),
            scanned,
        }
    }

    /// Remplaçant de `track`, une erreur réseau valant « aucun » (journalisée).
    async fn replacement_ok(&self, track: &Track) -> std::result::Result<Option<Track>, ()> {
        Ok(self.replacement(track).await.unwrap_or_else(|e| {
            tracing::warn!("remplaçant de {} introuvable : {e}", track.id);
            None
        }))
    }

    /// Même enregistrement par l'ISRC, sinon même chanson dans une recherche.
    async fn replacement(&self, track: &Track) -> Result<Option<Track>> {
        if let Some(isrc) = &track.isrc {
            let params = [("isrc", isrc.clone()), ("limit", "50".into())];
            let page: RawPage<RawTrack> = self.get("tracks", &params).await?;
            if let Some(found) = rules::pick_replacement(track, readable(page.items), true) {
                return Ok(Some(found));
            }
        }
        let artist = track.artists.first().map_or("", |a| a.name.as_str());
        let query = format!("{artist} {}", track.title);
        let params = [
            ("query", query),
            ("types", "TRACKS".to_owned()),
            ("limit", SEARCH_LIMIT.to_string()),
        ];
        let page: RawSearchTracks = self.get("search", &params).await?;
        Ok(rules::pick_replacement(
            track,
            readable(page.tracks.items),
            false,
        ))
    }

    /// Applique les changements choisis dans « Nettoyer » (positions de la liste lue), de la
    /// dernière position à la première. Refusé si le plan est incohérent.
    pub async fn clean_playlist(&self, uuid: &str, chosen: Vec<ListOp>) -> Result<ListOpsResult> {
        let list = ops::forward(chosen).map_err(|e| TidalError::Conflict(e.into()))?;
        Ok(self.apply_playlist_ops(uuid, &list).await)
    }

    /// Applique `list` à la playlist dans l'ordre donné, jusqu'à la première erreur. Un titre
    /// que le service refuse de rajouter (retiré du catalogue) est sauté, et les positions
    /// suivantes recalées (`ops::Skips`).
    pub async fn apply_playlist_ops(&self, uuid: &str, list: &[ListOp]) -> ListOpsResult {
        let mut applied = Vec::new();
        let mut skips = ops::Skips::default();
        let mut error = None;
        for &op in list {
            let adjusted = skips.adjust(op);
            match self.apply_playlist_op(uuid, adjusted).await {
                Ok(true) => applied.push(adjusted),
                Ok(false) => skips.note(&op),
                Err(e) => {
                    error = Some(e.to_string());
                    break;
                }
            }
        }
        ListOpsResult {
            applied: applied.len(),
            skipped: skips.count,
            undo: ops::undo(&applied),
            error,
        }
    }

    /// `false` : titre refusé par le service, rien n'a changé.
    async fn apply_playlist_op(&self, uuid: &str, op: ListOp) -> Result<bool> {
        match op {
            ListOp::Remove { index, track_id } => self
                .remove_from_playlist(uuid, index, track_id)
                .await
                .map(|()| true),
            ListOp::Insert { index, track_id } => self.insert_checked(uuid, index, track_id).await,
            // Le remplaçant d'abord : un échec ne fait jamais perdre le titre d'origine.
            ListOp::Replace {
                index,
                track_id,
                with,
            } => {
                if !self.insert_checked(uuid, index, with).await? {
                    return Ok(false);
                }
                if let Err(e) = self.remove_from_playlist(uuid, index + 1, track_id).await {
                    if let Err(undo) = self.remove_from_playlist(uuid, index, with).await {
                        tracing::warn!("remplaçant ajouté puis non retiré : {undo}");
                    }
                    return Err(e);
                }
                Ok(true)
            }
        }
    }

    /// `false` si le service refuse le titre : introuvable (retiré du catalogue, 404) ou
    /// rien d'ajouté.
    async fn insert_checked(&self, uuid: &str, index: usize, id: TrackId) -> Result<bool> {
        refused_as_false(self.insert_in_playlist(uuid, index, id).await)
    }

    /// Applique `list` aux titres likés (positions ignorées), jusqu'à la première erreur. Un
    /// titre que le service refuse de liker est sauté.
    pub async fn apply_liked_ops(&self, list: &[ListOp]) -> ListOpsResult {
        let mut applied = Vec::new();
        let mut skipped = 0;
        let mut error = None;
        for &op in list {
            match self.apply_liked_op(op).await {
                Ok(true) => applied.push(op),
                Ok(false) => skipped += 1,
                Err(e) => {
                    error = Some(e.to_string());
                    break;
                }
            }
        }
        ListOpsResult {
            applied: applied.len(),
            skipped,
            undo: ops::undo(&applied),
            error,
        }
    }

    async fn apply_liked_op(&self, op: ListOp) -> Result<bool> {
        let like = |id| LikeTarget::Track { id };
        match op {
            ListOp::Remove { track_id, .. } => {
                self.set_liked(&like(track_id), false).await.map(|()| true)
            }
            ListOp::Insert { track_id, .. } => {
                refused_as_false(self.set_liked(&like(track_id), true).await.map(|()| true))
            }
            ListOp::Replace { track_id, with, .. } => {
                if !refused_as_false(self.set_liked(&like(with), true).await.map(|()| true))? {
                    return Ok(false);
                }
                self.set_liked(&like(track_id), false).await.map(|()| true)
            }
        }
    }
}

/// Un 404 sur un ajout : titre retiré du catalogue (`Track not found`), refusé.
fn refused_as_false(result: Result<bool>) -> Result<bool> {
    match result {
        Err(TidalError::Api { status: 404, .. }) => Ok(false),
        other => other,
    }
}

/// `search?types=TRACKS` : seule la liste des titres est lue.
#[derive(serde::Deserialize)]
struct RawSearchTracks {
    #[serde(default = "RawPage::empty")]
    tracks: RawPage<RawTrack>,
}

/// Titres lisibles d'une réponse.
fn readable(raw: Vec<RawTrack>) -> impl Iterator<Item = Track> {
    raw.into_iter()
        .filter(RawTrack::streamable)
        .filter_map(RawTrack::into_track)
}

/// Titres de la liste avec leur position dans la réponse (un titre illisible est sauté
/// sans décaler les autres) et leur disponibilité.
fn entries(raw: Vec<RawTrack>) -> Vec<Entry> {
    raw.into_iter()
        .enumerate()
        .filter_map(|(index, r)| {
            let available = r.streamable();
            r.into_track().map(|track| Entry {
                index,
                track,
                available,
            })
        })
        .collect()
}
