//! Other hi-res edition of the same recording (#18).
//!
//! A track without the `HIRES_LOSSLESS` tag often exists in 24 bits on another edition
//! (original album, compilation, reissue): same ISRC, other id. `tracks?isrc=` returns
//! them all with their tags in ~130 ms (checked on 2026-10-30, `live_hires`).
//! With no result, falls back to a twin album by the artist with different ISRCs (`twin.rs`).

use std::collections::HashMap;
use std::sync::RwLock;

use super::Result;
use super::api::TidalClient;
use super::models::{Track, TrackId};
use super::raw::{RawPage, RawTrack};
use super::twin::TwinCache;

/// Duration gap tolerated between two editions, in seconds.
const DURATION_TOLERANCE: u32 = 2;

/// Result of the search by original track, `None` included (« no hi-res edition »):
/// it is never redone during the session.
#[derive(Default)]
pub struct HiresCache {
    editions: RwLock<HashMap<TrackId, Option<Track>>>,
    pub(super) twins: TwinCache,
}

impl HiresCache {
    fn get(&self, id: TrackId) -> Option<Option<Track>> {
        self.editions.read().ok()?.get(&id).cloned()
    }

    fn insert(&self, id: TrackId, edition: Option<Track>) {
        if let Ok(mut map) = self.editions.write() {
            map.insert(id, edition);
        }
    }
}

impl TidalClient {
    /// Answer already known for `id`, without network: `Some(None)` = no hi-res edition.
    pub fn known_hires_edition(&self, id: TrackId) -> Option<Option<Track>> {
        self.hires.get(id)
    }

    /// Hi-res edition of the same recording as `track`, if it is not hi-res itself.
    /// Network errors are not cached.
    pub async fn hires_edition(&self, track: &Track) -> Result<Option<Track>> {
        if track.hires {
            return Ok(None);
        }
        if let Some(known) = self.hires.get(track.id) {
            return Ok(known);
        }
        let isrc = match &track.isrc {
            Some(isrc) => isrc.clone(),
            // Track restored from an earlier version (no ISRC): it is asked for again.
            None => {
                let raw: RawTrack = self.get(&format!("tracks/{}", track.id), &[]).await?;
                match raw.into_track().and_then(|t| t.isrc) {
                    Some(isrc) => isrc,
                    None => {
                        self.hires.insert(track.id, None);
                        return Ok(None);
                    }
                }
            }
        };
        // The fallback's discography (#55) loads at the same time: a track with no
        // hi-res edition at all does not wait for the two requests one after the other.
        let params = [("isrc", isrc.clone()), ("limit", "50".into())];
        let lookup = self.get::<RawPage<RawTrack>>("tracks", &params);
        let (page, ()) = tokio::join!(lookup, self.prefetch_twin_albums(track));
        let page = page?;
        let candidates = page
            .items
            .into_iter()
            .filter(RawTrack::streamable)
            .filter_map(RawTrack::into_track);
        let edition = match pick_edition(track, &isrc, candidates) {
            Some(found) => Some(found),
            // Hi-res master published separately, under other ISRCs (#55).
            None => self.twin_edition(track).await?,
        };
        self.hires.insert(track.id, edition.clone());
        Ok(edition)
    }
}

/// Best hi-res edition among `candidates`: same ISRC, close duration, preferably
/// on an album with the same title (the original album rather than a compilation) and with
/// the same explicit marking. On a tie, the API order.
fn pick_edition(
    original: &Track,
    isrc: &str,
    candidates: impl IntoIterator<Item = Track>,
) -> Option<Track> {
    candidates
        .into_iter()
        .filter(|c| {
            c.hires
                && c.id != original.id
                && c.isrc.as_deref() == Some(isrc)
                && c.duration.abs_diff(original.duration) <= DURATION_TOLERANCE
        })
        .min_by_key(|c| {
            (
                !c.album.title.eq_ignore_ascii_case(&original.album.title),
                c.explicit != original.explicit,
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::AlbumRef;

    fn track(id: u64, album: &str, hires: bool, duration: u32) -> Track {
        Track {
            id: TrackId(id),
            title: "Billie Jean".into(),
            version: None,
            duration,
            artists: Vec::new(),
            album: AlbumRef {
                id,
                title: album.into(),
                cover_url: None,
                cover_small_url: None,
                cover_large_url: None,
                color: None,
            },
            explicit: false,
            ai: false,
            hires,
            isrc: Some("USSM19902991".into()),
        }
    }

    fn pick(original: &Track, candidates: Vec<Track>) -> Option<u64> {
        pick_edition(original, "USSM19902991", candidates).map(|t| t.id.0)
    }

    #[test]
    fn takes_first_hires_edition() {
        let original = track(1, "Thriller 25", false, 294);
        let candidates = vec![
            track(1, "Thriller 25", false, 294),
            track(2, "Number Ones", false, 294),
            track(3, "Number Ones", true, 294),
            track(4, "Thriller 40", true, 294),
        ];
        assert_eq!(pick(&original, candidates), Some(3));
    }

    #[test]
    fn prefers_same_album_title() {
        let original = track(1, "Thriller", false, 294);
        let candidates = vec![
            track(2, "Number Ones", true, 294),
            track(3, "THRILLER", true, 294),
        ];
        assert_eq!(pick(&original, candidates), Some(3));
    }

    #[test]
    fn prefers_same_explicit_flag() {
        let original = track(1, "a", false, 200);
        let mut explicit = track(2, "b", true, 200);
        explicit.explicit = true;
        let candidates = vec![explicit, track(3, "c", true, 200)];
        assert_eq!(pick(&original, candidates), Some(3));
    }

    #[test]
    fn rejects_other_durations_and_isrcs() {
        let original = track(1, "a", false, 294);
        let mut other_isrc = track(2, "b", true, 294);
        other_isrc.isrc = Some("GBUM71029604".into());
        let candidates = vec![
            other_isrc,
            track(3, "c", true, 297),
            track(4, "d", true, 291),
        ];
        assert_eq!(pick(&original, candidates), None);
        assert_eq!(
            pick(&original, vec![track(5, "e", true, 296)]),
            Some(5),
            "a 2 s gap is still tolerated"
        );
    }

    #[test]
    fn unstreamable_candidates_are_skipped() {
        let raw: Vec<RawTrack> = serde_json::from_str(
            r#"[{"id":2,"title":"a","duration":10,"album":{"id":3,"title":"c"},"streamReady":false},
                {"id":3,"title":"a","duration":10,"album":{"id":3,"title":"c"},"allowStreaming":false},
                {"id":4,"title":"a","duration":10,"album":{"id":3,"title":"c"},
                 "streamReady":true,"allowStreaming":true,"isrc":"X"}]"#,
        )
        .unwrap();
        let ok: Vec<_> = raw
            .into_iter()
            .filter(RawTrack::streamable)
            .filter_map(RawTrack::into_track)
            .collect();
        assert_eq!(ok.len(), 1);
        assert_eq!(ok[0].isrc.as_deref(), Some("X"));
    }
}
