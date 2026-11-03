//! Album jumeau hi-res (#55) : certains labels publient le master hi-res comme un album à
//! part, au même titre, avec **d'autres ISRC** (Eagles, « Hotel California (2013
//! Remaster) » : `20767338` en 16/44,1, `68709104` en 24/192). `tracks?isrc=` ne le voit
//! pas, la recherche non plus (elle déduplique les éditions), mais la discographie de
//! l'artiste (`artists/{id}/albums`, ~300 ms) liste les deux (vérifié le 2026-10-30).
//!
//! Repli de `hires_edition` quand l'ISRC ne donne rien : albums hi-res de l'artiste
//! principal au titre identique, puis le titre de même nom et de même durée. Sert aussi au
//! lien « Cet album existe en Hi-Res » de la page album (#59).

use std::collections::HashMap;
use std::sync::RwLock;

use super::Result;
use super::api::TidalClient;
use super::models::{AlbumSummary, Track};
use super::raw::{RawAlbumFull, RawTrack};

/// Écart de durée toléré entre deux éditions, en secondes (comme pour l'ISRC).
const DURATION_TOLERANCE: u32 = 2;
/// Taille de page et plafond de la discographie parcourue.
const PAGE_SIZE: u32 = 100;
const MAX_ALBUMS: usize = 1_000;
const MAX_TRACKS: usize = 1_000;

/// Albums hi-res par artiste, et titres hi-res par album jumeau : un album entier ne coûte
/// qu'une recherche.
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
    /// Titre de `track` sur un album jumeau hi-res de son artiste principal, s'il existe.
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

    /// Album hi-res de l'artiste principal au même titre que l'album `id`, si celui-ci ne
    /// l'est pas (#59). Une requête d'album, puis la discographie (en cache par artiste).
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

    /// Charge la discographie de l'artiste principal en avance. Une erreur est ignorée :
    /// `twin_edition` refera la requête et la remontera.
    pub(super) async fn prefetch_twin_albums(&self, track: &Track) {
        if let Some(artist) = track.artists.first() {
            let _ = self.hires_albums(artist.id).await;
        }
    }

    /// Albums hi-res de l'artiste (albums, puis EP et singles).
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

    /// Titres hi-res et lisibles d'un album jumeau.
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

/// Titre comparable : casse et espaces ignorés.
fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Albums hi-res au même titre que l'album de `track`, autres que lui, même marquage
/// explicite d'abord.
fn twin_albums<'a>(track: &Track, albums: &'a [AlbumSummary]) -> Vec<&'a AlbumSummary> {
    same_title_albums(track.album.id, &track.album.title, track.explicit, albums)
}

/// Albums hi-res titrés `title` (casse et espaces ignorés), autres que l'album `id`, ceux
/// de marquage explicite `explicit` d'abord.
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

/// Titre de même nom, même version et durée proche ; à égalité, le même marquage explicite.
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
            "ni lui-même, ni un autre titre, ni un 16 bits"
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
