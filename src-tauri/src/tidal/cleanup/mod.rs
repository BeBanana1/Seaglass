//! « Clean up » a user's playlist or liked tracks (#382): duplicates and tracks
//! removed from the catalog, with a playable replacement when one is found.
//!
//! Checked on 2026-11-12 (`cargo test live_cleanup_probe -- --ignored --nocapture`):
//! `playlists/{uuid}/tracks` returns **all** tracks, unavailable ones included
//! (`streamReady` and `allowStreaming` at `false`): positions stay right.
//! `favorites/tracks` hides some (433 received out of 460 announced), and those answer
//! 404 to `tracks/{id}`: nothing to offer for them. Replacement: `tracks?isrc=`, otherwise a
//! search « artist title » (rules in `rules.rs`).

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

/// Replacement searches in flight at most.
const LOOKUPS_IN_FLIGHT: usize = 4;
/// Results read per replacement search.
const SEARCH_LIMIT: u32 = 20;
/// Favorites walked at most (like the library).
const LIKES_MAX: usize = super::catalog::LIBRARY_MAX;

impl TidalClient {
    /// Changes proposed for playlist `uuid`, read again in full (without the kept copy).
    pub async fn playlist_cleanup(&self, uuid: &str) -> Result<CleanupReport> {
        let path = format!("playlists/{uuid}/tracks");
        let raw = self
            .get_all::<RawTrack>(&path, &[], PAGE_SIZE, MAX_TRACKS)
            .await?;
        Ok(self.report(entries(raw)).await)
    }

    /// Changes proposed for liked tracks (most recent to oldest).
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
        // First playable position of each track: a replacement already in the list turns
        // the unavailable one into a plain duplicate.
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

    /// Replacement for `track`, a network error counting as « none » (logged).
    async fn replacement_ok(&self, track: &Track) -> std::result::Result<Option<Track>, ()> {
        Ok(self.replacement(track).await.unwrap_or_else(|e| {
            tracing::warn!("replacement for {} not found: {e}", track.id);
            None
        }))
    }

    /// Same recording by ISRC, otherwise the same song in a search.
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

    /// Applies the changes chosen in « Clean up » (positions of the list read), from the
    /// last position to the first. Refused if the plan is inconsistent.
    pub async fn clean_playlist(&self, uuid: &str, chosen: Vec<ListOp>) -> Result<ListOpsResult> {
        let list = ops::forward(chosen).map_err(|e| TidalError::Conflict(e.into()))?;
        Ok(self.apply_playlist_ops(uuid, &list).await)
    }

    /// Applies `list` to the playlist in the given order, until the first error. A track
    /// that the service refuses to add (removed from the catalog) is skipped, and the following
    /// positions are adjusted (`ops::Skips`).
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

    /// `false`: track refused by the service, nothing changed.
    async fn apply_playlist_op(&self, uuid: &str, op: ListOp) -> Result<bool> {
        match op {
            ListOp::Remove { index, track_id } => self
                .remove_from_playlist(uuid, index, track_id)
                .await
                .map(|()| true),
            ListOp::Insert { index, track_id } => self.insert_checked(uuid, index, track_id).await,
            // The replacement first: a failure never loses the original track.
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
                        tracing::warn!("replacement added then not removed: {undo}");
                    }
                    return Err(e);
                }
                Ok(true)
            }
        }
    }

    /// `false` if the service refuses the track: not found (removed from the catalog, 404) or
    /// nothing added.
    async fn insert_checked(&self, uuid: &str, index: usize, id: TrackId) -> Result<bool> {
        refused_as_false(self.insert_in_playlist(uuid, index, id).await)
    }

    /// Applies `list` to the liked tracks (positions ignored), until the first error. A
    /// track that the service refuses to like is skipped.
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

/// A 404 on an add: track removed from the catalog (`Track not found`), refused.
fn refused_as_false(result: Result<bool>) -> Result<bool> {
    match result {
        Err(TidalError::Api { status: 404, .. }) => Ok(false),
        other => other,
    }
}

/// `search?types=TRACKS`: only the list of tracks is read.
#[derive(serde::Deserialize)]
struct RawSearchTracks {
    #[serde(default = "RawPage::empty")]
    tracks: RawPage<RawTrack>,
}

/// Playable tracks of a response.
fn readable(raw: Vec<RawTrack>) -> impl Iterator<Item = Track> {
    raw.into_iter()
        .filter(RawTrack::streamable)
        .filter_map(RawTrack::into_track)
}

/// Tracks of the list with their position in the response (an unreadable track is skipped
/// without shifting the others) and their availability.
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
