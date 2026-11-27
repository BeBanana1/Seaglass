//! Ranking of the tracks of a search (#232). Pure logic, tested.
//!
//! For a common word ("yellow", "stay", "creep"), TIDAL interleaves, between the tracks
//! that bear this name, those by artists whose name contains it (Yellow Claw, Yellow Days,
//! Creep-P…): seen on 2026-11-06. So we move up, without changing TIDAL's order within
//! each group:
//! 1. the tracks whose name is the query,
//! 2. those whose name contains the query, or is contained in the query ("get lucky
//!    daft punk"),
//! 3. the rest.
//!
//! An artist search ("daft punk") finds no track of that name: TIDAL's order is kept as
//! is.

use super::models::Track;

/// Lowercase, letters and digits only, spaces collapsed; apostrophes
/// disparaissent (« don't » = « dont »).
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

/// Whole words: "stay" is in "stay with me", not in "stayin alive".
fn contains_words(haystack: &str, needle: &str) -> bool {
    !needle.is_empty() && format!(" {haystack} ").contains(&format!(" {needle} "))
}

fn tier(title: &str, query: &str) -> u8 {
    let title = normalize(title);
    if title == query {
        0
    } else if contains_words(&title, query) || contains_words(query, &title) {
        1
    } else {
        2
    }
}

/// Tracks whose name matches the query first; TIDAL's order kept otherwise.
pub fn titles_first(tracks: &mut [Track], query: &str) {
    let query = normalize(query);
    if query.is_empty() {
        return;
    }
    // Stable sort: at equal rank, TIDAL's order (relevance, popularity) stays.
    tracks.sort_by_key(|t| tier(&t.title, &query));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::{AlbumRef, ArtistRef, TrackId};

    fn track(id: u64, title: &str, artist: &str) -> Track {
        Track {
            id: TrackId(id),
            title: title.into(),
            version: None,
            duration: 200,
            artists: vec![ArtistRef {
                id,
                name: artist.into(),
            }],
            album: AlbumRef {
                id,
                title: "Album".into(),
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
    fn titles_named_like_the_query_come_before_artists_named_like_it() {
        // Real TIDAL order for "yellow" and "stay" (2026-11-06).
        let mut yellow = vec![
            track(1, "Yellow", "Coldplay"),
            track(2, "A Little While", "Yellow Days"),
            track(3, "CRUSH", "Yellow Claw"),
            track(4, "Yellow Magic (Tong Poo)", "Yellow Magic Orchestra"),
        ];
        titles_first(&mut yellow, "Yellow");
        assert_eq!(ids(&yellow), vec![1, 4, 2, 3]);

        let mut stay = vec![
            track(1, "STAY", "The Kid LAROI"),
            track(2, "2 Cups", "Stay Flee Get Lizzy"),
            track(3, "Stay", "Rihanna"),
            track(4, "stay", "all things break"),
            track(5, "Stayin' Alive", "Bee Gees"),
        ];
        titles_first(&mut stay, " stay ");
        assert_eq!(ids(&stay), vec![1, 3, 4, 2, 5]);
    }

    #[test]
    fn artist_searches_keep_tidal_order() {
        let mut tracks = vec![
            track(1, "One More Time", "Daft Punk"),
            track(2, "Around the World", "Daft Punk"),
            track(3, "Get Lucky", "Daft Punk"),
        ];
        titles_first(&mut tracks, "daft punk");
        assert_eq!(ids(&tracks), vec![1, 2, 3]);
    }

    #[test]
    fn title_inside_a_longer_query_counts() {
        let mut tracks = vec![
            track(1, "Instant Crush", "Daft Punk"),
            track(2, "Get Lucky (feat. Pharrell Williams)", "Daft Punk"),
            track(3, "Get Lucky", "Daft Punk"),
        ];
        titles_first(&mut tracks, "get lucky daft punk");
        assert_eq!(ids(&tracks), vec![3, 1, 2]);
        // Punctuation, apostrophes and case ignored.
        for query in ["dont stop me now", "Don’t stop me now!"] {
            let mut tracks = vec![
                track(1, "Other", "X"),
                track(2, "Don't Stop Me Now", "Queen"),
            ];
            titles_first(&mut tracks, query);
            assert_eq!(ids(&tracks), vec![2, 1]);
        }
    }
}
