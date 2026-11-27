//! Blocked tracks and artists (#334), stored at TIDAL: they apply on every
//! device, and the service's radios already take them into account.
//!
//! Endpoints checked on 2026-11-12 with the PKCE client: `users/{id}/blocks/tracks` and
//! `…/artists` (paged, `{ created, item, type }` like favorites), add with `POST`
//! (form `trackId` / `artistId`, 200), remove with `DELETE …/{id}` (204), both
//! idempotent.
//!
//! The ids are kept in memory to filter mixes and radios (`mix.rs`): a block made just
//! now, or a mix not yet regenerated, would get through otherwise. Nothing
//! else is filtered: a blocked track stays playable from an album or a playlist.

use std::collections::HashSet;
use std::sync::Mutex;

use reqwest::Method;

use super::Result;
use super::api::{API_V1, TidalClient};
use super::catalog::PAGE_SIZE;
use super::models::{ArtistSummary, BlockTarget, Blocked, Track, TrackId};
use super::raw::{RawArtistFull, RawFavorite, RawTrack};

/// Cap of each list (a handful of items in practice).
const BLOCKS_MAX: usize = 5_000;

#[derive(Default)]
struct Sets {
    tracks: HashSet<TrackId>,
    artists: HashSet<u64>,
}

impl Sets {
    fn allows(&self, track: &Track) -> bool {
        !self.tracks.contains(&track.id)
            && !track.artists.iter().any(|a| self.artists.contains(&a.id))
    }
}

/// Blocked ids; `None` until the lists have been read.
#[derive(Default)]
pub struct Blocks {
    sets: Mutex<Option<Sets>>,
}

impl Blocks {
    fn loaded(&self) -> bool {
        self.sets.lock().is_ok_and(|s| s.is_some())
    }

    fn load(&self, blocked: &Blocked) {
        if let Ok(mut guard) = self.sets.lock() {
            *guard = Some(Sets {
                tracks: blocked.tracks.iter().map(|t| t.id).collect(),
                artists: blocked.artists.iter().map(|a| a.id).collect(),
            });
        }
    }

    /// Applies a block or an unblock, if the lists are loaded.
    fn set(&self, target: &BlockTarget, blocked: bool) {
        let Ok(mut guard) = self.sets.lock() else {
            return;
        };
        let Some(sets) = guard.as_mut() else {
            return;
        };
        match (target, blocked) {
            (BlockTarget::Track { id }, true) => sets.tracks.insert(*id),
            (BlockTarget::Track { id }, false) => sets.tracks.remove(id),
            (BlockTarget::Artist { id }, true) => sets.artists.insert(*id),
            (BlockTarget::Artist { id }, false) => sets.artists.remove(id),
        };
    }

    /// Removes blocked tracks and those of a blocked artist (main or featured).
    fn retain(&self, tracks: &mut Vec<Track>) {
        if let Ok(guard) = self.sets.lock()
            && let Some(sets) = guard.as_ref()
        {
            tracks.retain(|t| sets.allows(t));
        }
    }

    pub fn clear(&self) {
        if let Ok(mut guard) = self.sets.lock() {
            *guard = None;
        }
    }
}

impl TidalClient {
    /// Both lists, from the most recent block to the oldest; updates the sets.
    pub async fn blocked(&self) -> Result<Blocked> {
        let user = self.user_id().await?;
        let (tracks_path, artists_path) = (
            format!("users/{user}/blocks/tracks"),
            format!("users/{user}/blocks/artists"),
        );
        let (tracks, artists) = tokio::try_join!(
            self.get_all::<RawFavorite<RawTrack>>(&tracks_path, &[], PAGE_SIZE, BLOCKS_MAX),
            self.get_all::<RawFavorite<RawArtistFull>>(&artists_path, &[], PAGE_SIZE, BLOCKS_MAX),
        )?;
        let blocked = Blocked {
            tracks: tracks
                .into_iter()
                .filter_map(|f| f.item.into_track())
                .collect(),
            artists: artists
                .into_iter()
                .map(|f| ArtistSummary::from(f.item))
                .collect(),
        };
        self.blocks.load(&blocked);
        Ok(blocked)
    }

    pub async fn set_blocked(&self, target: &BlockTarget, blocked: bool) -> Result<()> {
        let user = self.user_id().await?;
        let (kind, field, id) = match target {
            BlockTarget::Track { id } => ("tracks", "trackId", id.0),
            BlockTarget::Artist { id } => ("artists", "artistId", *id),
        };
        if blocked {
            let form = [(field, id.to_string())];
            let path = format!("users/{user}/blocks/{kind}");
            self.write(Method::POST, API_V1, &path, &[], Some(&form), None)
                .await?;
        } else {
            let path = format!("users/{user}/blocks/{kind}/{id}");
            self.write(Method::DELETE, API_V1, &path, &[], None, None)
                .await?;
        }
        self.blocks.set(target, blocked);
        Ok(())
    }

    /// `tracks` without what is blocked. The lists are read on the first call; if they
    /// are unavailable, nothing is removed (the service's radios already filter) and the
    /// read will be retried on the next call.
    pub async fn without_blocked(&self, mut tracks: Vec<Track>) -> Vec<Track> {
        if !self.blocks.loaded()
            && let Err(e) = self.blocked().await
        {
            tracing::warn!("blocked tracks and artists unreadable: {e}");
        }
        self.blocks.retain(&mut tracks);
        tracks
    }
}

#[cfg(test)]
mod tests {
    use super::{Blocks, Sets};
    use crate::tidal::models::{AlbumRef, ArtistRef, BlockTarget, Blocked, Track, TrackId};

    fn track(id: u64, artists: &[u64]) -> Track {
        Track {
            id: TrackId(id),
            title: format!("t{id}"),
            version: None,
            duration: 200,
            artists: artists
                .iter()
                .map(|&id| ArtistRef {
                    id,
                    name: format!("a{id}"),
                })
                .collect(),
            album: AlbumRef {
                id: 1,
                title: "album".into(),
                cover_url: None,
                cover_small_url: None,
                cover_large_url: None,
                color: None,
            },
            explicit: false,
            ai: false,
            hires: false,
            isrc: None,
        }
    }

    fn ids(tracks: &[Track]) -> Vec<u64> {
        tracks.iter().map(|t| t.id.0).collect()
    }

    #[test]
    fn nothing_is_removed_before_loading() {
        let blocks = Blocks::default();
        blocks.set(&BlockTarget::Track { id: TrackId(1) }, true);
        let mut tracks = vec![track(1, &[10])];
        blocks.retain(&mut tracks);
        assert_eq!(ids(&tracks), [1]);
    }

    #[test]
    fn blocked_tracks_and_artists_are_removed() {
        let blocks = Blocks::default();
        blocks.load(&Blocked {
            tracks: vec![track(1, &[10])],
            artists: vec![],
        });
        blocks.set(&BlockTarget::Artist { id: 20 }, true);
        // Featured artist blocked: the track goes too.
        let mut tracks = vec![track(1, &[10]), track(2, &[10, 20]), track(3, &[30])];
        blocks.retain(&mut tracks);
        assert_eq!(ids(&tracks), [3]);

        blocks.set(&BlockTarget::Track { id: TrackId(1) }, false);
        blocks.set(&BlockTarget::Artist { id: 20 }, false);
        let mut tracks = vec![track(1, &[10]), track(2, &[10, 20])];
        blocks.retain(&mut tracks);
        assert_eq!(ids(&tracks), [1, 2]);
    }

    #[test]
    fn clear_forgets_everything() {
        let blocks = Blocks::default();
        blocks.load(&Blocked::default());
        assert!(blocks.loaded());
        blocks.clear();
        assert!(!blocks.loaded());
        assert!(Sets::default().allows(&track(1, &[1])));
    }
}
