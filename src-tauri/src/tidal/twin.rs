//! Hi-res twin album (#55): some labels publish the hi-res master as a separate album,
//! with the same title and **other ISRCs** (Eagles, "Hotel California (2013
//! Remaster)": `20767338` in 16/44.1, `68709104` in 24/192). `tracks?isrc=` does not see
//! it, nor does search (it deduplicates editions), but the artist's discography
//! (`artists/{id}/albums`, ~300 ms) lists both (checked on 2026-10-30).
//!
//! Fallback of `hires_edition` when the ISRC gives nothing: hi-res albums of the main
//! artist with the identical title, then the track of the same name and duration. Also
//! serves the "This album exists in Hi-Res" link of the album page (#59).

use std::collections::HashMap;
use std::sync::RwLock;

use super::Result;
use super::api::TidalClient;
use super::models::{AlbumSummary, Track};
use super::raw::{RawAlbumFull, RawTrack};

/// Duration gap tolerated between two editions, in seconds (as for the ISRC).
const DURATION_TOLERANCE: u32 = 2;
/// Page size and cap of the discography walked.
const PAGE_SIZE: u32 = 100;
const MAX_ALBUMS: usize = 1_000;
const MAX_TRACKS: usize = 1_000;

/// Hi-res albums per artist, and hi-res tracks per twin album: a whole album costs only
/// one lookup.
#[derive(Default)]
pub struct TwinCache {
    artists: RwLock<HashMap<u64, Vec<AlbumSummary>>>,
    albums: RwLock<HashMap<u64, Vec<Track>>>,
}

impl TwinCache {
    fn artist(&self, id: u64) -> Option<Vec<AlbumSummary>> {
        self.artists.read().ok()?.get(&id).cloned()
    }

    fn album(&self, id: u64) -> Option<Vec<Track>> {
        self.albums.read().ok()?.get(&id).cloned()
    }

    fn insert_artist(&self, id: u64, albums: Vec<AlbumSummary>) {
        if let Ok(mut map) = self.artists.write() {
            map.insert(id, albums);
        }
    }

    fn insert_album(&self, id: u64, tracks: Vec<Track>) {
        if let Ok(mut map) = self.albums.write() {
            map.insert(id, tracks);
        }
    }
}

impl TidalClient {
    /// Track `track` on a hi-res twin album of its main artist, if there is one.
    pub(super) async fn twin_edition(&self, track: &Track) -> Result<Option<Track>> {
        let Some(artist) = track.artists.first() else {
            return Ok(None);
        };
        let albums = self.hires_albums(artist.id).await?;
        for album in twin_albums(track, &albums) {
            let tracks = self.hires_album_tracks(album.id).await?;
            if let Some(found) = pick_twin_track(track, tracks) {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    /// Hi-res album of the main artist with the same title as album `id`, if that one is not
    /// hi-res (#59). One album request, then the discography (cached per artist).
    pub async fn hires_twin_album(&self, id: u64) -> Result<Option<AlbumSummary>> {
        let raw: RawAlbumFull = self.get(&format!("albums/{id}"), &[]).await?;
        let album = AlbumSummary::from(raw);
        let Some(artist) = album.artists.first().filter(|_| !album.hires) else {
            return Ok(None);
        };
        let albums = self.hires_albums(artist.id).await?;
        let twins = same_title_albums(album.id, &album.title, album.explicit, &albums);
        Ok(twins.first().map(|a| (*a).clone()))
    }

    /// Loads the main artist's discography ahead of time. An error is ignored:
    /// `twin_edition` will redo the request and surface it.
    pub(super) async fn prefetch_twin_albums(&self, track: &Track) {
        if let Some(artist) = track.artists.first() {
            let _ = self.hires_albums(artist.id).await;
        }
    }

    /// Hi-res albums of the artist (albums, then EPs and singles).
    async fn hires_albums(&self, artist: u64) -> Result<Vec<AlbumSummary>> {
        if let Some(known) = self.hires.twins.artist(artist) {
            return Ok(known);
        }
        let path = format!("artists/{artist}/albums");
        let singles = [("filter", "EPSANDSINGLES".to_owned())];
        let (albums, singles) = tokio::try_join!(
            self.get_all::<RawAlbumFull>(&path, &[], PAGE_SIZE, MAX_ALBUMS),
            self.get_all::<RawAlbumFull>(&path, &singles, PAGE_SIZE, MAX_ALBUMS),
        )?;
        let hires: Vec<AlbumSummary> = albums
            .into_iter()
            .chain(singles)
            .map(AlbumSummary::from)
            .filter(|a| a.hires)
            .collect();
        self.hires.twins.insert_artist(artist, hires.clone());
        Ok(hires)
    }

    /// Hi-res, playable tracks of a twin album.
    async fn hires_album_tracks(&self, album: u64) -> Result<Vec<Track>> {
        if let Some(known) = self.hires.twins.album(album) {
            return Ok(known);
        }
        let raw: Vec<RawTrack> = self
            .get_all(
                &format!("albums/{album}/tracks"),
                &[],
                PAGE_SIZE,
                MAX_TRACKS,
            )
            .await?;
        let tracks: Vec<Track> = raw
            .into_iter()
            .filter(RawTrack::streamable)
            .filter_map(RawTrack::into_track)
            .filter(|t| t.hires)
            .collect();
        self.hires.twins.insert_album(album, tracks.clone());
        Ok(tracks)
    }
}

/// Comparable title: case and spaces ignored.
fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Hi-res albums with the same title as the album of `track`, other than it, same
/// explicit marking first.
fn twin_albums<'a>(track: &Track, albums: &'a [AlbumSummary]) -> Vec<&'a AlbumSummary> {
    same_title_albums(track.album.id, &track.album.title, track.explicit, albums)
}

/// Hi-res albums titled `title` (case and spaces ignored), other than album `id`, those
/// with explicit marking `explicit` first.
fn same_title_albums<'a>(
    id: u64,
    title: &str,
    explicit: bool,
    albums: &'a [AlbumSummary],
) -> Vec<&'a AlbumSummary> {
    let title = normalize(title);
    let mut twins: Vec<&AlbumSummary> = albums
        .iter()
        .filter(|a| a.hires && a.id != id && normalize(&a.title) == title)
        .collect();
    twins.sort_by_key(|a| a.explicit != explicit);
    twins
}

/// Track of the same name, same version and close duration; on a tie, the same explicit
/// marking.
fn pick_twin_track(original: &Track, candidates: Vec<Track>) -> Option<Track> {
    let title = normalize(&original.title);
    let version = original
        .version
        .as_deref()
        .map(normalize)
        .unwrap_or_default();
    candidates
        .into_iter()
        .filter(|c| {
            c.hires
                && c.id != original.id
                && normalize(&c.title) == title
                && c.version.as_deref().map(normalize).unwrap_or_default() == version
                && c.duration.abs_diff(original.duration) <= DURATION_TOLERANCE
        })
        .min_by_key(|c| c.explicit != original.explicit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::{AlbumRef, ArtistRef, TrackId};

    fn track(id: u64, title: &str, album: (u64, &str), hires: bool, duration: u32) -> Track {
        Track {
            id: TrackId(id),
            title: title.into(),
            version: None,
            duration,
            artists: vec![ArtistRef {
                id: 16_139,
                name: "Eagles".into(),
            }],
            album: AlbumRef {
                id: album.0,
                title: album.1.into(),
                cover_url: None,
                cover_small_url: None,
                cover_large_url: None,
                color: None,
            },
            explicit: false,
            ai: false,
            hires,
            isrc: Some("USEE11300353".into()),
        }
    }

    fn album(id: u64, title: &str, hires: bool) -> AlbumSummary {
        AlbumSummary {
            id,
            title: title.into(),
            artists: Vec::new(),
            cover_url: None,
            cover_large_url: None,
            color: None,
            release_date: None,
            number_of_tracks: None,
            album_type: None,
            explicit: false,
            ai: false,
            hires,
        }
    }

    const REMASTER: &str = "Hotel California (2013 Remaster)";

    #[test]
    fn finds_hires_album_with_same_title() {
        let original = track(20_767_339, REMASTER, (20_767_338, REMASTER), false, 391);
        let albums = vec![
            album(20_767_338, REMASTER, true),
            album(
                81_127_510,
                "Hotel California (40th Anniversary Expanded Edition)",
                true,
            ),
            album(68_709_104, "hotel  california (2013 remaster)", true),
            album(1, REMASTER, false),
        ];
        let twins: Vec<u64> = twin_albums(&original, &albums)
            .iter()
            .map(|a| a.id)
            .collect();
        assert_eq!(
            twins,
            vec![68_709_104],
            "neither itself, nor another track, nor a 16-bit one"
        );
    }

    #[test]
    fn prefers_same_explicit_album() {
        let original = track(1, "a", (1, "X"), false, 200);
        let mut explicit = album(2, "X", true);
        explicit.explicit = true;
        let albums = vec![explicit, album(3, "X", true)];
        let ids: Vec<u64> = twin_albums(&original, &albums)
            .iter()
            .map(|a| a.id)
            .collect();
        assert_eq!(ids, vec![3, 2]);
    }

    #[test]
    fn album_twin_ignores_itself_and_cd_editions() {
        let albums = vec![
            album(20_767_338, REMASTER, false),
            album(68_709_104, REMASTER, true),
        ];
        let twins: Vec<u64> = same_title_albums(20_767_338, REMASTER, false, &albums)
            .iter()
            .map(|a| a.id)
            .collect();
        assert_eq!(twins, vec![68_709_104]);
        assert!(same_title_albums(68_709_104, REMASTER, false, &albums).is_empty());
    }

    #[test]
    fn picks_track_by_title_and_duration() {
        let original = track(20_767_339, REMASTER, (20_767_338, REMASTER), false, 391);
        let twin = (68_709_104, REMASTER);
        let candidates = vec![
            track(
                68_709_106,
                "New Kid in Town (2013 Remaster)",
                twin,
                true,
                304,
            ),
            track(68_709_107, REMASTER, twin, true, 420),
            track(68_709_105, REMASTER, twin, true, 393),
        ];
        let found = pick_twin_track(&original, candidates).map(|t| t.id.0);
        assert_eq!(found, Some(68_709_105));
    }

    #[test]
    fn version_must_match() {
        let mut original = track(1, "Song", (1, "X"), false, 200);
        original.version = Some("Live".into());
        let studio = track(2, "Song", (2, "X"), true, 200);
        assert_eq!(pick_twin_track(&original, vec![studio.clone()]), None);
        let mut live = studio;
        live.version = Some(" live ".into());
        assert_eq!(
            pick_twin_track(&original, vec![live]).map(|t| t.id.0),
            Some(2)
        );
    }

    #[test]
    fn non_hires_candidates_are_ignored() {
        let original = track(1, "Song", (1, "X"), false, 200);
        assert_eq!(
            pick_twin_track(&original, vec![track(2, "Song", (2, "X"), false, 200)]),
            None
        );
    }
}
