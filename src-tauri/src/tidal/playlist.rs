//! Playlist pages, kept in memory with their ETag (#152).
//!
//! Reading a playlist means its header (`playlists/{uuid}`) plus all the pages of
//! `playlists/{uuid}/tracks`: 0.6-0.85 s for 484 tracks. The header's ETag changes on
//! every modification (checked by `cargo test live_playlist_etag -- --ignored`): a page
//! already read is revalidated by the header alone, and its tracks are only read again if
//! the ETag changed. The UI first shows the kept copy (`cached_playlist`), then asks
//! whether the playlist changed since (`playlist_since`). Pages also survive a restart,
//! in the database (`commands::snapshots`, #181), and come back here through
//! `restore_playlist`.

use std::sync::Mutex;

use super::api::{API_V1, Call, TidalClient};
use super::catalog::{MAX_TRACKS, PAGE_SIZE};
use super::models::PlaylistPage;
#[cfg(desktop)]
use super::models::TrackEntry;
use super::raw::{self, RawPlaylist, RawTrack};
use super::{Result, TidalError};

/// Most playlists kept.
const MAX_PLAYLISTS: usize = 20;
/// Most tracks kept, all playlists together (the most recent one is always kept).
const MAX_CACHED_TRACKS: usize = 20_000;

impl TidalClient {
    /// Full page of the playlist, revalidated by its ETag if it is already kept.
    /// The UI goes through `playlist_since`; used by the remote control (#336) and the
    /// live tests.
    pub async fn playlist(&self, uuid: &str) -> Result<PlaylistPage> {
        self.playlist_since(uuid, None).await?.ok_or_else(|| {
            TidalError::Decode(format!("playlists/{uuid}: page missing without a revision"))
        })
    }

    /// Kept copy, no network.
    pub fn cached_playlist(&self, uuid: &str) -> Option<PlaylistPage> {
        let page = self.playlists.get(uuid)?;
        // Its tracks go back into the metadata cache: playback can start before the
        // revalidation.
        self.cache.insert_all(page.tracks.iter().map(|e| &e.track));
        Some(page)
    }

    /// Copy read back from disk at the next launch (#181): kept as a page that was read,
    /// so that ETag revalidation reuses its tracks if they did not change.
    pub fn restore_playlist(&self, uuid: &str, page: PlaylistPage) {
        if page.revision.is_none() {
            return;
        }
        self.cache.insert_all(page.tracks.iter().map(|e| &e.track));
        self.playlists.put(uuid, page);
    }

    /// `None` if the playlist did not change since `known` (its `revision` already shown),
    /// otherwise the up-to-date page.
    pub async fn playlist_since(
        &self,
        uuid: &str,
        known: Option<&str>,
    ) -> Result<Option<PlaylistPage>> {
        let cached = self.playlists.get(uuid);
        if known.is_none() && cached.is_none() {
            // Nothing to revalidate: header and tracks in parallel.
            let tracks_path = format!("playlists/{uuid}/tracks");
            let ((head, etag), tracks) = tokio::try_join!(
                self.playlist_head(uuid),
                self.get_all::<RawTrack>(&tracks_path, &[], PAGE_SIZE, MAX_TRACKS),
            )?;
            return Ok(Some(self.keep(uuid, head, etag, tracks).await));
        }
        let (head, etag) = self.playlist_head(uuid).await?;
        if etag.is_some() && etag.as_deref() == known {
            return Ok(None);
        }
        let user = self.user_id().await.ok();
        if let Some(mut page) = cached.filter(|c| c.revision.is_some() && c.revision == etag) {
            // Same tracks; the header read again is taken over (track count, image).
            page.playlist = head.into_summary(user);
            self.cache.insert_all(page.tracks.iter().map(|e| &e.track));
            return Ok(Some(page));
        }
        let tracks_path = format!("playlists/{uuid}/tracks");
        let tracks = self
            .get_all::<RawTrack>(&tracks_path, &[], PAGE_SIZE, MAX_TRACKS)
            .await?;
        Ok(Some(self.keep(uuid, head, etag, tracks).await))
    }

    /// Tracks of a playlist in order, for the library export (#330): the kept copy if it is
    /// up to date, otherwise a read that does not replace the kept pages (an export of 200
    /// playlists would flush the cache of the 20 last opened).
    #[cfg(desktop)]
    pub async fn playlist_tracks(&self, uuid: &str) -> Result<Vec<TrackEntry>> {
        if self.playlists.get(uuid).is_some() {
            return Ok(self.playlist(uuid).await?.tracks);
        }
        let tracks_path = format!("playlists/{uuid}/tracks");
        let tracks = self
            .get_all::<RawTrack>(&tracks_path, &[], PAGE_SIZE, MAX_TRACKS)
            .await?;
        Ok(raw::playlist_entries(tracks))
    }

    /// Header of the playlist and its ETag.
    async fn playlist_head(&self, uuid: &str) -> Result<(RawPlaylist, Option<String>)> {
        let path = format!("playlists/{uuid}");
        let resp = self.call(&Call::get(API_V1, &path, &[])).await?;
        let etag = resp
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let head = resp
            .json()
            .await
            .map_err(|e| TidalError::Decode(format!("{path} : {e}")))?;
        Ok((head, etag))
    }

    async fn keep(
        &self,
        uuid: &str,
        head: RawPlaylist,
        etag: Option<String>,
        tracks: Vec<RawTrack>,
    ) -> PlaylistPage {
        let user = self.user_id().await.ok();
        let page = PlaylistPage {
            playlist: head.into_summary(user),
            tracks: self.remember_entries(raw::playlist_entries(tracks)),
            revision: etag,
        };
        // Without an ETag, nothing would tell whether the copy is still right.
        if page.revision.is_some() {
            self.playlists.put(uuid, page.clone());
        } else {
            self.playlists.remove(uuid);
        }
        page
    }
}

/// Last pages read, the most recent first.
#[derive(Default)]
pub(super) struct PlaylistCache {
    entries: Mutex<Vec<(String, PlaylistPage)>>,
}

impl PlaylistCache {
    fn get(&self, uuid: &str) -> Option<PlaylistPage> {
        let mut entries = self.entries.lock().ok()?;
        let i = entries.iter().position(|(u, _)| u == uuid)?;
        let entry = entries.remove(i);
        let page = entry.1.clone();
        entries.insert(0, entry);
        Some(page)
    }

    fn put(&self, uuid: &str, page: PlaylistPage) {
        let Ok(mut entries) = self.entries.lock() else {
            return;
        };
        entries.retain(|(u, _)| u != uuid);
        entries.insert(0, (uuid.to_owned(), page));
        trim(&mut entries);
    }

    /// Playlist modified by the app: the kept copy is worth nothing anymore.
    pub(super) fn remove(&self, uuid: &str) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.retain(|(u, _)| u != uuid);
        }
    }

    /// Sign-out: `mine` depends on the account.
    pub(super) fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }
}

/// Keeps at most `MAX_PLAYLISTS` pages and `MAX_CACHED_TRACKS` tracks, starting from the
/// most recent; the first is kept even if it alone exceeds the track limit.
fn trim(entries: &mut Vec<(String, PlaylistPage)>) {
    let mut total = 0;
    let keep = entries
        .iter()
        .take(MAX_PLAYLISTS)
        .position(|(_, p)| {
            total += p.tracks.len();
            total > MAX_CACHED_TRACKS
        })
        .map_or(MAX_PLAYLISTS, |i| i.max(1));
    entries.truncate(keep);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::{AlbumRef, PlaylistSummary, Track, TrackEntry, TrackId};

    fn page(uuid: &str, tracks: usize) -> PlaylistPage {
        let track = Track {
            id: TrackId(1),
            title: "t".into(),
            version: None,
            duration: 100,
            artists: Vec::new(),
            album: AlbumRef {
                id: 1,
                title: "a".into(),
                cover_url: None,
                cover_small_url: None,
                cover_large_url: None,
                color: None,
            },
            explicit: false,
            ai: false,
            hires: false,
            isrc: None,
        };
        PlaylistPage {
            playlist: PlaylistSummary {
                uuid: uuid.to_owned(),
                title: uuid.to_owned(),
                description: None,
                image_url: None,
                number_of_tracks: None,
                duration: None,
                editorial: false,
                mine: false,
            },
            tracks: vec![
                TrackEntry {
                    track,
                    added_at: None,
                };
                tracks
            ],
            revision: Some(format!("etag-{uuid}")),
        }
    }

    fn uuids(cache: &PlaylistCache) -> Vec<String> {
        let entries = cache.entries.lock().unwrap();
        entries.iter().map(|(u, _)| u.clone()).collect()
    }

    #[test]
    fn get_moves_to_front() {
        let cache = PlaylistCache::default();
        cache.put("a", page("a", 1));
        cache.put("b", page("b", 1));
        assert_eq!(uuids(&cache), ["b", "a"]);
        assert_eq!(cache.get("a").unwrap().revision.as_deref(), Some("etag-a"));
        assert_eq!(uuids(&cache), ["a", "b"]);
        assert!(cache.get("c").is_none());
    }

    #[test]
    fn put_replaces_and_remove_forgets() {
        let cache = PlaylistCache::default();
        cache.put("a", page("a", 1));
        cache.put("b", page("b", 1));
        cache.put("a", page("a", 3));
        assert_eq!(uuids(&cache), ["a", "b"]);
        assert_eq!(cache.get("a").unwrap().tracks.len(), 3);
        cache.remove("a");
        assert_eq!(uuids(&cache), ["b"]);
        cache.clear();
        assert!(uuids(&cache).is_empty());
    }

    #[test]
    fn bounded_by_count() {
        let cache = PlaylistCache::default();
        for i in 0..MAX_PLAYLISTS + 5 {
            cache.put(&i.to_string(), page(&i.to_string(), 1));
        }
        let kept = uuids(&cache);
        assert_eq!(kept.len(), MAX_PLAYLISTS);
        assert_eq!(kept[0], (MAX_PLAYLISTS + 4).to_string());
    }

    #[test]
    fn bounded_by_tracks_keeping_the_latest() {
        let cache = PlaylistCache::default();
        cache.put("a", page("a", MAX_CACHED_TRACKS / 2));
        cache.put("b", page("b", MAX_CACHED_TRACKS / 2));
        assert_eq!(uuids(&cache), ["b", "a"]);
        cache.put("c", page("c", 1));
        assert_eq!(uuids(&cache), ["c", "b"]);
        // A playlist larger than the limit is still kept, alone.
        cache.put("huge", page("huge", MAX_CACHED_TRACKS + 1));
        assert_eq!(uuids(&cache), ["huge"]);
    }
}
