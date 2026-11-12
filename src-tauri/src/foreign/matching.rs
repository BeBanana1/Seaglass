//! Choix des titres et albums TIDAL qui correspondent à un lien d'un autre service (#229).
//! Logique pure, testée : un tri stable garde l'ordre de TIDAL à score égal.

use super::meta::{Wanted, first_artist, strip_decorations};
use crate::tidal::models::{AlbumSummary, ArtistRef, Track};

/// Résultats montrés au plus pour un lien.
pub const MAX_MATCHES: usize = 10;
/// Écart de durée toléré entre deux services, en secondes.
const DURATION_SLACK: u32 = 3;

fn normalize(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '\'' | '\u{2019}'))
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_lowercase().next().unwrap_or(c)
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn title_score(found: &str, wanted: &str) -> u32 {
    let (found, wanted) = (
        normalize(&strip_decorations(found)),
        normalize(&strip_decorations(wanted)),
    );
    if found.is_empty() || wanted.is_empty() {
        0
    } else if found == wanted {
        4
    } else if found.contains(&wanted) || wanted.contains(&found) {
        2
    } else {
        0
    }
}

fn artist_score(found: &[ArtistRef], wanted: Option<&str>) -> u32 {
    let Some(wanted) = wanted
        .map(|a| normalize(first_artist(a)))
        .filter(|a| !a.is_empty())
    else {
        return 0;
    };
    let hit = found.iter().any(|a| {
        let name = normalize(&a.name);
        !name.is_empty() && (name == wanted || name.contains(&wanted) || wanted.contains(&name))
    });
    if hit { 2 } else { 0 }
}

/// Titres d'une recherche classés par ressemblance avec le titre voulu ; seuls ceux dont
/// le nom ressemble sont gardés s'il y en a.
pub fn rank_tracks(mut tracks: Vec<Track>, wanted: &Wanted) -> Vec<Track> {
    let score = |t: &Track| {
        let duration = wanted
            .duration
            .is_some_and(|d| t.duration.abs_diff(d) <= DURATION_SLACK);
        let album = wanted
            .album
            .as_deref()
            .is_some_and(|a| normalize(&t.album.title) == normalize(a));
        title_score(&t.title, &wanted.title)
            + artist_score(&t.artists, wanted.artist.as_deref())
            + u32::from(duration)
            + u32::from(album)
    };
    tracks.sort_by_key(|t| std::cmp::Reverse(score(t)));
    if tracks
        .iter()
        .any(|t| title_score(&t.title, &wanted.title) > 0)
    {
        tracks.retain(|t| title_score(&t.title, &wanted.title) > 0);
    }
    tracks.truncate(MAX_MATCHES);
    tracks
}

/// Éditions d'un même ISRC : celle de l'album du lien d'abord, compilations ensuite.
pub fn rank_editions(mut tracks: Vec<Track>, album: Option<&str>) -> Vec<Track> {
    let album = album.map(normalize);
    tracks.sort_by_key(|t| {
        let same = album
            .as_deref()
            .is_some_and(|a| normalize(&t.album.title) == a);
        u8::from(!same)
    });
    tracks.truncate(MAX_MATCHES);
    tracks
}

pub fn rank_albums(mut albums: Vec<AlbumSummary>, wanted: &Wanted) -> Vec<AlbumSummary> {
    let score = |a: &AlbumSummary| {
        title_score(&a.title, &wanted.title) + artist_score(&a.artists, wanted.artist.as_deref())
    };
    albums.sort_by_key(|a| std::cmp::Reverse(score(a)));
    if albums
        .iter()
        .any(|a| title_score(&a.title, &wanted.title) > 0)
    {
        albums.retain(|a| title_score(&a.title, &wanted.title) > 0);
    }
    albums.truncate(MAX_MATCHES);
    albums
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::{AlbumRef, TrackId};

    fn track(id: u64, title: &str, artist: &str, album: &str, duration: u32) -> Track {
        Track {
            id: TrackId(id),
            title: title.into(),
            version: None,
            duration,
            artists: vec![ArtistRef {
                id,
                name: artist.into(),
            }],
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
            hires: false,
            isrc: None,
        }
    }

    fn ids(tracks: &[Track]) -> Vec<u64> {
        tracks.iter().map(|t| t.id.0).collect()
    }

    #[test]
    fn the_matching_title_and_artist_come_first() {
        let wanted = Wanted {
            title: "Get Lucky (feat. Pharrell Williams and Nile Rodgers)".into(),
            artist: Some("Daft Punk, Pharrell Williams, Nile Rodgers".into()),
            album: Some("Random Access Memories".into()),
            duration: Some(370),
        };
        let found = vec![
            track(
                1,
                "Instant Crush",
                "Daft Punk",
                "Random Access Memories",
                337,
            ),
            track(2, "Get Lucky", "Halestorm", "Reanimate 2.0", 247),
            track(3, "Get Lucky (Radio Edit)", "Daft Punk", "Get Lucky", 248),
            track(
                4,
                "Get Lucky (feat. Pharrell Williams & Nile Rodgers)",
                "Daft Punk",
                "Random Access Memories",
                369,
            ),
        ];
        // Le titre sans rapport disparaît ; la version de l'album du lien passe devant.
        assert_eq!(ids(&rank_tracks(found, &wanted)), vec![4, 3, 2]);
    }

    #[test]
    fn editions_of_the_link_album_come_first() {
        let found = vec![
            track(
                1,
                "Get Lucky",
                "Daft Punk",
                "Decade of Summer: The 10s",
                369,
            ),
            track(2, "Get Lucky", "Daft Punk", "Random Access Memories", 369),
        ];
        assert_eq!(
            ids(&rank_editions(found, Some("Random Access Memories"))),
            vec![2, 1]
        );
    }

    #[test]
    fn nothing_alike_keeps_tidal_order() {
        let wanted = Wanted {
            title: "Inconnu".into(),
            ..Wanted::default()
        };
        let found = vec![track(1, "A", "X", "Y", 1), track(2, "B", "X", "Y", 1)];
        assert_eq!(ids(&rank_tracks(found, &wanted)), vec![1, 2]);
    }
}
