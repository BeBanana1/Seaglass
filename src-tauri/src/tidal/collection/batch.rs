//! Batch writes for the multiple selection (#332), checked by
//! `cargo test live_batch -- --ignored --nocapture`.
//!
//! - Favorites: POST v1 `users/{id}/favorites/tracks` accepts several comma-separated
//!   `trackIds`; DELETE `…/favorites/tracks/{a,b,c}` removes several tracks at once.
//! - Playlist: DELETE v1 `playlists/{uuid}/items/{i,j,k}` removes several positions in one
//!   request (a single version of the playlist, so a single ETag).

use reqwest::Method;

use super::super::api::API_V1;
use super::super::models::{LikeTarget, TrackId};
use super::super::raw::RawPage;
use super::super::{Result, TidalClient, TidalError};
// `retry_stale!` uses the last three.
use super::{RawItem, STALE_ATTEMPTS, STALE_DELAY, stale_to_conflict};

/// Tracks per request (favorites, removed positions): the id ends up in the path or
/// the form, so we stay far from URL limits.
const CHUNK: usize = 100;
/// Tracks read per page to check positions before a removal.
const PAGE: usize = 100;

impl TidalClient {
    /// Adds or removes several tracks from the favorites. Tracks already in the requested
    /// state are ignored by the caller; here everything is sent.
    pub async fn set_tracks_liked(&self, ids: &[TrackId], liked: bool) -> Result<()> {
        let user = self.user_id().await?;
        for chunk in ids.chunks(CHUNK) {
            let joined = join(chunk);
            if liked {
                let form = [
                    ("trackIds", joined),
                    ("onArtifactNotFound", "FAIL".to_owned()),
                ];
                let path = format!("users/{user}/favorites/tracks");
                self.write(Method::POST, API_V1, &path, &[], Some(&form), None)
                    .await?;
            } else {
                let path = format!("users/{user}/favorites/tracks/{joined}");
                self.write(Method::DELETE, API_V1, &path, &[], None, None)
                    .await?;
            }
            for id in chunk {
                self.likes.set(&LikeTarget::Track { id: *id }, liked);
            }
        }
        Ok(())
    }

    /// Removes several tracks from a playlist, designated by their position (0 = first) and
    /// the track expected at that position: everything is refused if one of them has changed.
    pub async fn remove_many_from_playlist(
        &self,
        uuid: &str,
        items: &[(usize, TrackId)],
    ) -> Result<()> {
        let mut items = items.to_vec();
        items.sort_unstable_by_key(|(index, _)| *index);
        items.dedup_by_key(|(index, _)| *index);
        // From bottom to top, in batches: a removed batch does not shift the positions of the
        // following batches, higher up in the playlist.
        for chunk in items.rchunks(CHUNK) {
            let indices: Vec<String> = chunk.iter().map(|(i, _)| i.to_string()).collect();
            let path = format!("playlists/{uuid}/items/{}", indices.join(","));
            let etag = retry_stale!(
                async {
                    let etag = self.checked_etag_many(uuid, chunk).await?;
                    self.write_raw(Method::DELETE, API_V1, &path, &[], None, Some(&etag))
                        .await
                        .map(|_| etag)
                }
                .await
            )?;
            self.settle(uuid, &etag).await;
        }
        Ok(())
    }

    /// Like `checked_etag`, for several sorted positions: the tracks are read again in
    /// pages between the first and the last position.
    async fn checked_etag_many(&self, uuid: &str, items: &[(usize, TrackId)]) -> Result<String> {
        self.playlists.remove(uuid);
        let etag = self.fresh_etag(uuid).await?;
        let (Some(first), Some(last)) = (items.first(), items.last()) else {
            return Ok(etag);
        };
        let path = format!("playlists/{uuid}/items");
        let mut found: Vec<TrackId> = Vec::new();
        let mut offset = first.0;
        while offset <= last.0 {
            let query = [("offset", offset.to_string()), ("limit", PAGE.to_string())];
            let page: RawPage<RawItem> = self.get(&path, &query).await?;
            if page.items.is_empty() {
                break;
            }
            offset += page.items.len();
            found.extend(page.items.into_iter().map(|i| i.item.id));
        }
        let at = |index: usize| found.get(index - first.0).copied();
        if items.iter().any(|(index, id)| at(*index) != Some(*id)) {
            return Err(TidalError::Conflict(
                crate::i18n::tr(
                    "la playlist a changé, rouvrez-la avant de la modifier",
                    "the playlist has changed, reopen it before editing it",
                )
                .into(),
            ));
        }
        Ok(etag)
    }

    /// Undo of a batch removal: each track goes back to its position, from the
    /// smallest to the largest (each put-back restores the positions of the following ones).
    /// Returns the number of tracks put back.
    pub async fn restore_many_to_playlist(
        &self,
        uuid: &str,
        items: &[(usize, TrackId)],
    ) -> Result<usize> {
        let mut items = items.to_vec();
        items.sort_unstable_by_key(|(index, _)| *index);
        let mut restored = 0;
        for (index, id) in items {
            if self.insert_in_playlist(uuid, index, id).await? {
                restored += 1;
            }
        }
        Ok(restored)
    }
}

fn join(ids: &[TrackId]) -> String {
    ids.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}
