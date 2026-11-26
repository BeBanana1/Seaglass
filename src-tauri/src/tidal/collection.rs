//! Writes to the user's library: playlists here, favorites (#4, #81) in `favorites.rs`.
//! Checked on a real account by `cargo test live_collection -- --ignored --nocapture`.
//!
//! - Favorites (#4, #81): GET v1 `users/{id}/favorites/ids` (every id, as strings), POST
//!   `users/{id}/favorites/{tracks,albums,artists,playlists}` (`trackIds`, `albumIds`,
//!   `artistIds`, `uuids`), DELETE `…/<type>/{id}`. Mixes (#204): GET v2
//!   `favorites/mixes/ids`, PUT v2 `favorites/mixes/add` / `remove` (`mixIds` in the query).
//! - Playlist: created with PUT v2 `my-collection/playlists/folders/create-playlist`,
//!   deleted with PUT v2 `my-collection/playlists/folders/remove` (`trns=trn:playlist:<uuid>`).
//! - A playlist's tracks: POST v1 `playlists/{uuid}/items` (answer `addedItemIds`), DELETE
//!   `…/items/{index}`, POST `…/items/{index}` (`toIndex`, move, #201), with the ETag of
//!   `playlists/{uuid}` as `If-None-Match`. Title and description: POST v1
//!   `playlists/{uuid}` (#201).
//! - Batch writes of the multiple selection (#332): `batch.rs`.

use reqwest::Method;
use serde::Deserialize;

use super::api::{API_V1, API_V2, Call};
use super::models::{PlaylistSummary, TrackId};
use super::raw::RawPage;
use super::versions;
use super::{Result, TidalClient, TidalError};

/// Attempts of a playlist change refused for a stale ETag.
const STALE_ATTEMPTS: usize = 3;
const STALE_DELAY: std::time::Duration = std::time::Duration::from_millis(400);

/// Retries a change (ETag read included) refused with a 412. Right after a write,
/// `playlists/{uuid}` sometimes still returns the old ETag (seen on 2026-11-05, #201: two
/// writes in a row failed); a few hundred ms are enough. A macro rather than a function
/// taking an async closure: that one broke the `Send` inference of the Tauri commands.
macro_rules! retry_stale {
    ($op:expr) => {{
        let mut attempt = 1;
        loop {
            match $op {
                Err(TidalError::Api { status: 412, .. }) if attempt < STALE_ATTEMPTS => {
                    tracing::debug!("stale ETag, new attempt ({attempt})");
                    tokio::time::sleep(STALE_DELAY).await;
                    attempt += 1;
                }
                other => break other.map_err(stale_to_conflict),
            }
        }
    }};
}

// After the macro: `batch` uses it.
mod batch;
mod favorites;

/// ETag reads while waiting for an up-to-date replica (`fresh_etag`, `settle`).
const VERSION_ATTEMPTS: usize = 6;
const VERSION_DELAY: std::time::Duration = std::time::Duration::from_millis(250);

/// Tracks sent per add request (the API's `limit`).
const ADD_CHUNK: usize = 100;

impl TidalClient {
    /// The user's playlists, the only ones they can change.
    pub async fn own_playlists(&self) -> Result<Vec<PlaylistSummary>> {
        let mut playlists = self.library_playlists().await?.items;
        playlists.retain(|p| p.mine);
        Ok(playlists)
    }

    /// Creates a playlist at the root of the library and returns its uuid.
    pub async fn create_playlist(&self, title: &str, description: &str) -> Result<String> {
        let query = [
            ("name", title.to_owned()),
            ("description", description.to_owned()),
            ("folderId", "root".to_owned()),
        ];
        let path = "my-collection/playlists/folders/create-playlist";
        let created: RawCreated = self
            .write(Method::PUT, API_V2, path, &query, None, None)
            .await?
            .json()
            .await
            .map_err(|e| TidalError::Decode(format!("{path} : {e}")))?;
        Ok(created.data.uuid)
    }

    /// Deletes a playlist of the user (#201).
    pub async fn delete_playlist(&self, uuid: &str) -> Result<()> {
        self.playlists.remove(uuid);
        let query = [("trns", format!("trn:playlist:{uuid}"))];
        let path = "my-collection/playlists/folders/remove";
        self.write(Method::PUT, API_V2, path, &query, None, None)
            .await
            .map(drop)
    }

    /// Adds tracks at the end of a playlist, in order, without duplicates.
    /// Returns the number of tracks actually added (0 if they were all there).
    pub async fn add_to_playlist(&self, uuid: &str, ids: &[TrackId]) -> Result<usize> {
        // Kept copy cleared before writing: even a partial failure makes it wrong.
        self.playlists.remove(uuid);
        let path = format!("playlists/{uuid}/items");
        let mut added = 0;
        for chunk in ids.chunks(ADD_CHUNK) {
            let ids: Vec<String> = chunk.iter().map(ToString::to_string).collect();
            let form = [
                ("trackIds", ids.join(",")),
                ("onArtifactNotFound", "FAIL".to_owned()),
                ("onDupes", "SKIP".to_owned()),
            ];
            // Each addition changes the ETag: it is read again before each request.
            let (resp, etag) = retry_stale!(
                async {
                    let etag = self.fresh_etag(uuid).await?;
                    self.write_raw(Method::POST, API_V1, &path, &[], Some(&form), Some(&etag))
                        .await
                        .map(|r| (r, etag))
                }
                .await
            )?;
            self.settle(uuid, &etag).await;
            let resp: RawAdded = resp
                .json()
                .await
                .map_err(|e| TidalError::Decode(format!("{path} : {e}")))?;
            added += resp.added_item_ids.len();
        }
        Ok(added)
    }

    /// Puts a track back at position `index` (0 = first): undoing a removal (#349).
    /// `onDupes=ADD`: the track comes back even if it is elsewhere in the playlist.
    /// Returns `false` if nothing was added.
    pub async fn insert_in_playlist(&self, uuid: &str, index: usize, id: TrackId) -> Result<bool> {
        self.playlists.remove(uuid);
        let path = format!("playlists/{uuid}/items");
        let form = [
            ("trackIds", id.to_string()),
            ("onArtifactNotFound", "FAIL".to_owned()),
            ("onDupes", "ADD".to_owned()),
            ("toIndex", index.to_string()),
        ];
        let (resp, etag) = retry_stale!(
            async {
                let etag = self.fresh_etag(uuid).await?;
                self.write_raw(Method::POST, API_V1, &path, &[], Some(&form), Some(&etag))
                    .await
                    .map(|r| (r, etag))
            }
            .await
        )?;
        self.settle(uuid, &etag).await;
        let resp: RawAdded = resp
            .json()
            .await
            .map_err(|e| TidalError::Decode(format!("{path} : {e}")))?;
        Ok(!resp.added_item_ids.is_empty())
    }

    /// Removes the track at this position (0 = first), after checking that it is indeed
    /// `expected`: the playlist may have changed elsewhere since it was shown.
    pub async fn remove_from_playlist(
        &self,
        uuid: &str,
        index: usize,
        expected: TrackId,
    ) -> Result<()> {
        let path = format!("playlists/{uuid}/items/{index}");
        let etag = retry_stale!(
            async {
                let etag = self.checked_etag(uuid, index, expected).await?;
                self.write_raw(Method::DELETE, API_V1, &path, &[], None, Some(&etag))
                    .await
                    .map(|_| etag)
            }
            .await
        )?;
        self.settle(uuid, &etag).await;
        Ok(())
    }

    /// Moves track `from` so that it ends up at position `to` (#201): POST v1
    /// `playlists/{uuid}/items/{from}` with `toIndex` **in the form** (in the query, it is
    /// ignored without an error). Past the end, the track goes to the end.
    pub async fn move_in_playlist(
        &self,
        uuid: &str,
        from: usize,
        to: usize,
        expected: TrackId,
    ) -> Result<()> {
        let path = format!("playlists/{uuid}/items/{from}");
        let form = [("toIndex", to.to_string())];
        let etag = retry_stale!(
            async {
                let etag = self.checked_etag(uuid, from, expected).await?;
                self.write_raw(Method::POST, API_V1, &path, &[], Some(&form), Some(&etag))
                    .await
                    .map(|_| etag)
            }
            .await
        )?;
        self.settle(uuid, &etag).await;
        Ok(())
    }

    /// Renames and changes the description (#201): POST v1 `playlists/{uuid}` (form `title`,
    /// `description`), visible at once.
    pub async fn edit_playlist(&self, uuid: &str, title: &str, description: &str) -> Result<()> {
        self.playlists.remove(uuid);
        let path = format!("playlists/{uuid}");
        let form = [
            ("title", title.to_owned()),
            ("description", description.to_owned()),
        ];
        let etag = retry_stale!(
            async {
                let etag = self.fresh_etag(uuid).await?;
                self.write_raw(Method::POST, API_V1, &path, &[], Some(&form), Some(&etag))
                    .await
                    .map(|_| etag)
            }
            .await
        )?;
        self.settle(uuid, &etag).await;
        Ok(())
    }

    /// Kept copy cleared, then the playlist's ETag, after checking that the track at `index`
    /// is indeed `expected`. ETag read before the check: a change between the two gives a
    /// 412.
    async fn checked_etag(&self, uuid: &str, index: usize, expected: TrackId) -> Result<String> {
        self.playlists.remove(uuid);
        let etag = self.fresh_etag(uuid).await?;
        let query = [("offset", index.to_string()), ("limit", "1".to_owned())];
        let page: RawPage<RawItem> = self.get(&format!("playlists/{uuid}/items"), &query).await?;
        if page.items.first().map(|i| i.item.id) != Some(expected) {
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

    /// ETag to send with a change: never older than the last version seen after one of the
    /// app's writes (`versions.rs`); a replica lagging behind is read again.
    async fn fresh_etag(&self, uuid: &str) -> Result<String> {
        for _ in 0..VERSION_ATTEMPTS {
            let etag = self.playlist_etag(uuid).await?;
            if !self.versions.is_stale(uuid, &etag) {
                return Ok(etag);
            }
            tracing::debug!("ETag behind the last write, read again");
            tokio::time::sleep(VERSION_DELAY).await;
        }
        Err(stale_to_conflict(TidalError::Api {
            status: 412,
            message: String::new(),
        }))
    }

    /// After a write sent with `before`: waits to see the new version and keeps it, so the
    /// next change does not start from an older state.
    async fn settle(&self, uuid: &str, before: &str) {
        for _ in 0..VERSION_ATTEMPTS {
            match self.playlist_etag(uuid).await {
                Ok(after) if versions::is_newer(&after, before) => {
                    self.versions.note(uuid, &after);
                    return;
                }
                Ok(_) => tokio::time::sleep(VERSION_DELAY).await,
                Err(e) => {
                    tracing::debug!("unreadable version after the write: {e}");
                    return;
                }
            }
        }
        tracing::debug!("new playlist version not visible yet");
    }

    /// Current version of the playlist, required by every change.
    pub(super) async fn playlist_etag(&self, uuid: &str) -> Result<String> {
        let path = format!("playlists/{uuid}");
        let resp = self.call(&Call::get(API_V1, &path, &[])).await?;
        resp.headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
            .ok_or_else(|| TidalError::Decode(format!("{path}: no ETag")))
    }

    /// Write; a refused ETag (412) becomes `TidalError::Conflict`.
    pub(super) async fn write(
        &self,
        method: Method,
        base: &str,
        path: &str,
        query: &[(&str, String)],
        form: Option<&[(&str, String)]>,
        if_none_match: Option<&str>,
    ) -> Result<reqwest::Response> {
        self.write_raw(method, base, path, query, form, if_none_match)
            .await
            .map_err(stale_to_conflict)
    }

    /// Write that lets the 412 through, for `retry_stale`.
    async fn write_raw(
        &self,
        method: Method,
        base: &str,
        path: &str,
        query: &[(&str, String)],
        form: Option<&[(&str, String)]>,
        if_none_match: Option<&str>,
    ) -> Result<reqwest::Response> {
        self.call(&Call {
            method,
            base,
            path,
            query,
            form,
            json: None,
            if_none_match,
            headers: &[],
        })
        .await
    }
}

fn stale_to_conflict(e: TidalError) -> TidalError {
    match e {
        TidalError::Api { status: 412, .. } => TidalError::Conflict(
            crate::i18n::tr(
                "la playlist a été modifiée ailleurs, réessayez",
                "the playlist was changed elsewhere, try again",
            )
            .into(),
        ),
        other => other,
    }
}

#[derive(Deserialize)]
struct RawCreated {
    data: RawCreatedPlaylist,
}

#[derive(Deserialize)]
struct RawCreatedPlaylist {
    uuid: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAdded {
    #[serde(default)]
    added_item_ids: Vec<u64>,
}

#[derive(Deserialize)]
struct RawItem {
    item: RawItemId,
}

#[derive(Deserialize)]
struct RawItemId {
    id: TrackId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn added_ids_empty_on_duplicate() {
        let raw: RawAdded =
            serde_json::from_str(r#"{"lastUpdated":1793366927853,"addedItemIds":[]}"#).unwrap();
        assert!(raw.added_item_ids.is_empty());
        let raw: RawAdded =
            serde_json::from_str(r#"{"lastUpdated":1,"addedItemIds":[156749594,156749595]}"#)
                .unwrap();
        assert_eq!(raw.added_item_ids.len(), 2);
    }
}
