//! Cleanup rules (#382), without network: duplicates, choice of a replacement.
//!
//! Two tracks are the **same song** if they have the same id, or a close duration (±2 s) and
//! either the same ISRC (same recording), or the same main artist, the same title and
//! the same version once the edition marks are removed (« 2011 Remaster », « Single
//! Version »…). A « Live » version or a remix remain separate songs.

use std::collections::HashMap;

use crate::tidal::models::{CleanupItem, Track, TrackId};

/// Duration gap tolerated between two editions, in seconds (like `hires.rs`).
const DURATION_TOLERANCE: u32 = 2;
/// Marks of a simple reissue, removed from the title and the version before comparing.
const EDITION_MARKERS: [&str; 3] = ["remaster", "single version", "album version"];

/// Track of a list with its position and availability.
#[derive(Debug, Clone)]
pub struct Entry {
    pub index: usize,
    pub track: Track,
    pub available: bool,
}

/// Comparable title or version: lowercase, spaces collapsed, edition marks removed
/// (in parentheses, in brackets or after « - »).
pub(super) fn comparable(text: &str) -> String {
    let lower = text.to_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut rest = lower.as_str();
    while let Some(open) = rest.find(['(', '[']) {
        let close_char = if rest[open..].starts_with('(') {
            ')'
        } else {
            ']'
        };
        let Some(len) = rest[open..].find(close_char) else {
            break;
        };
        let group = &rest[open..open + len + 1];
        out.push_str(&rest[..open]);
        if !has_marker(group) {
            out.push_str(group);
        }
        rest = &rest[open + len + 1..];
    }
    out.push_str(rest);
    let kept: Vec<&str> = out
        .split(" - ")
        .enumerate()
        .filter(|(i, part)| *i == 0 || !has_marker(part))
        .map(|(_, part)| part)
        .collect();
    kept.join(" - ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn has_marker(text: &str) -> bool {
    EDITION_MARKERS.iter().any(|m| text.contains(m))
}

/// Comparable main artist, title and version.
fn song_key(track: &Track) -> (u64, String, String) {
    (
        track.artists.first().map_or(0, |a| a.id),
        comparable(&track.title),
        track.version.as_deref().map(comparable).unwrap_or_default(),
    )
}

fn close_durations(a: &Track, b: &Track) -> bool {
    a.duration.abs_diff(b.duration) <= DURATION_TOLERANCE
}

fn same_isrc(a: &Track, b: &Track) -> bool {
    matches!((&a.isrc, &b.isrc), (Some(x), Some(y)) if x.eq_ignore_ascii_case(y))
}

pub(super) fn same_song(a: &Track, b: &Track) -> bool {
    a.id == b.id || (close_durations(a, b) && (same_isrc(a, b) || song_key(a) == song_key(b)))
}

/// Duplicates to remove, and positions of the remaining unavailable tracks (to replace).
///
/// In each group of one song, the first playable occurrence is kept (the
/// first one at all if none is playable); the others are duplicates.
pub fn duplicates(entries: &[Entry]) -> (Vec<CleanupItem>, Vec<usize>) {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    // Index into the candidate groups: avoids comparing each track to all the others.
    let mut by_id: HashMap<TrackId, Vec<usize>> = HashMap::new();
    let mut by_isrc: HashMap<String, Vec<usize>> = HashMap::new();
    let mut by_key: HashMap<(u64, String, String), Vec<usize>> = HashMap::new();
    for (i, entry) in entries.iter().enumerate() {
        let track = &entry.track;
        let isrc = track.isrc.as_deref().map(str::to_uppercase);
        let key = song_key(track);
        let mut candidates: Vec<usize> = by_id.get(&track.id).cloned().unwrap_or_default();
        if let Some(found) = isrc.as_ref().and_then(|s| by_isrc.get(s)) {
            candidates.extend(found);
        }
        if let Some(found) = by_key.get(&key) {
            candidates.extend(found);
        }
        candidates.sort_unstable();
        candidates.dedup();
        let group = candidates
            .into_iter()
            .find(|&g| {
                groups[g]
                    .iter()
                    .any(|&m| same_song(&entries[m].track, track))
            })
            .unwrap_or_else(|| {
                groups.push(Vec::new());
                groups.len() - 1
            });
        groups[group].push(i);
        let register = |list: &mut Vec<usize>| {
            if !list.contains(&group) {
                list.push(group);
            }
        };
        register(by_id.entry(track.id).or_default());
        if let Some(isrc) = isrc {
            register(by_isrc.entry(isrc).or_default());
        }
        register(by_key.entry(key).or_default());
    }

    let mut items = Vec::new();
    let mut to_replace = Vec::new();
    for group in groups {
        let keeper = group
            .iter()
            .copied()
            .find(|&m| entries[m].available)
            .unwrap_or(group[0]);
        if !entries[keeper].available {
            to_replace.push(keeper);
        }
        for &m in group.iter().filter(|&&m| m != keeper) {
            items.push(CleanupItem::Duplicate {
                index: entries[m].index,
                track: entries[m].track.clone(),
                kept_index: entries[keeper].index,
                kept: Box::new(entries[keeper].track.clone()),
                same_track: entries[m].track.id == entries[keeper].track.id,
            });
        }
    }
    to_replace.sort_unstable();
    (items, to_replace)
}

/// Best playable replacement of an unavailable track: same ISRC (`by_isrc`) or same
/// song (artist, title and version), close duration. Preferred: the same album title, the
/// same explicit marking, then hi-res. On a tie, the API order.
pub fn pick_replacement(
    original: &Track,
    candidates: impl IntoIterator<Item = Track>,
    by_isrc: bool,
) -> Option<Track> {
    let key = song_key(original);
    candidates
        .into_iter()
        .filter(|c| {
            c.id != original.id
                && close_durations(c, original)
                && if by_isrc {
                    same_isrc(c, original)
                } else {
                    song_key(c) == key
                }
        })
        .min_by_key(|c| {
            (
                !c.album.title.eq_ignore_ascii_case(&original.album.title),
                c.explicit != original.explicit,
                !c.hires,
            )
        })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::tidal::models::{AlbumRef, ArtistRef};

    pub(crate) fn track(id: u64, title: &str, isrc: Option<&str>, duration: u32) -> Track {
        Track {
            id: TrackId(id),
            title: title.into(),
            version: None,
            duration,
            artists: vec![ArtistRef {
                id: 7,
                name: "Artist".into(),
            }],
            album: AlbumRef {
                id: id * 10,
                title: "Album".into(),
                cover_url: None,
                cover_small_url: None,
                cover_large_url: None,
                color: None,
            },
            explicit: false,
            ai: false,
            hires: false,
            isrc: isrc.map(Into::into),
        }
    }

    fn entries(list: Vec<(Track, bool)>) -> Vec<Entry> {
        list.into_iter()
            .enumerate()
            .map(|(index, (track, available))| Entry {
                index,
                track,
                available,
            })
            .collect()
    }

    fn removed(items: &[CleanupItem]) -> Vec<(usize, usize, bool)> {
        let mut out: Vec<_> = items
            .iter()
            .filter_map(|i| match i {
                CleanupItem::Duplicate {
                    index,
                    kept_index,
                    same_track,
                    ..
                } => Some((*index, *kept_index, *same_track)),
                CleanupItem::Unavailable { .. } => None,
            })
            .collect();
        out.sort_unstable();
        out
    }

    #[test]
    fn edition_markers_are_ignored() {
        assert_eq!(comparable("Song (2011 Remaster)"), "song");
        assert_eq!(comparable("Song - Remastered 2009"), "song");
        assert_eq!(comparable("Song [Single Version]"), "song");
        assert_eq!(comparable("Song  (Live)"), "song (live)");
        assert_eq!(
            comparable("Song - Live at Wembley"),
            "song - live at wembley"
        );
        assert_eq!(comparable("Remaster Blaster"), "remaster blaster");
        assert_eq!(comparable("Song (unclosed"), "song (unclosed");
    }

    #[test]
    fn same_song_rules() {
        let a = track(1, "Song", Some("X1"), 200);
        assert!(same_song(&a, &track(1, "Other", None, 10)), "same id");
        assert!(
            same_song(&a, &track(2, "Renamed", Some("x1"), 201)),
            "same ISRC"
        );
        assert!(
            !same_song(&a, &track(2, "Song", Some("X1"), 230)),
            "duration too far"
        );
        assert!(
            same_song(&a, &track(3, "Song (2011 Remaster)", Some("X2"), 198)),
            "remaster"
        );
        let mut live = track(4, "Song", Some("X3"), 200);
        live.version = Some("Live".into());
        assert!(!same_song(&a, &live), "a live version is a different song");
        let mut other_artist = track(5, "Song", None, 200);
        other_artist.artists[0].id = 8;
        assert!(!same_song(&a, &other_artist), "reprise d'un autre artiste");
    }

    #[test]
    fn keeps_first_and_flags_copies() {
        let list = entries(vec![
            (track(1, "A", Some("I1"), 100), true),
            (track(2, "B", Some("I2"), 100), true),
            (track(1, "A", Some("I1"), 100), true),
            (track(3, "A (Remaster)", Some("I9"), 101), true),
        ]);
        let (items, to_replace) = duplicates(&list);
        assert_eq!(removed(&items), vec![(2, 0, true), (3, 0, false)]);
        assert!(to_replace.is_empty());
    }

    #[test]
    fn prefers_an_available_copy() {
        let list = entries(vec![
            (track(1, "A", Some("I1"), 100), false),
            (track(2, "A", Some("I1"), 100), true),
            (track(3, "C", Some("I3"), 100), false),
        ]);
        let (items, to_replace) = duplicates(&list);
        assert_eq!(removed(&items), vec![(0, 1, false)]);
        assert_eq!(
            to_replace,
            vec![2],
            "only the unavailable one without a playable copy"
        );
    }

    #[test]
    fn unavailable_group_keeps_first_for_replacement() {
        let list = entries(vec![
            (track(1, "A", Some("I1"), 100), false),
            (track(1, "A", Some("I1"), 100), false),
        ]);
        let (items, to_replace) = duplicates(&list);
        assert_eq!(removed(&items), vec![(1, 0, true)]);
        assert_eq!(to_replace, vec![0]);
    }

    #[test]
    fn positions_come_from_entries() {
        // An unreadable track (no album) was skipped: positions stay those of the API.
        let list = vec![
            Entry {
                index: 0,
                track: track(1, "A", None, 100),
                available: true,
            },
            Entry {
                index: 2,
                track: track(1, "A", None, 100),
                available: true,
            },
        ];
        let (items, _) = duplicates(&list);
        assert_eq!(removed(&items), vec![(2, 0, true)]);
    }

    #[test]
    fn change_json_carries_its_op() {
        use crate::tidal::models::CleanupChange;
        let item = CleanupItem::Unavailable {
            index: 4,
            track: track(1, "A", None, 100),
            replacement: Some(Box::new(track(2, "A", None, 100))),
        };
        let json = serde_json::to_value(CleanupChange::from(item)).unwrap();
        assert_eq!(json["kind"], "unavailable");
        assert_eq!(json["index"], 4);
        assert_eq!(json["replacement"]["id"], 2);
        assert_eq!(
            json["op"],
            serde_json::json!({"op": "replace", "index": 4, "trackId": 1, "with": 2})
        );
        let dup = CleanupItem::Duplicate {
            index: 3,
            track: track(1, "A", None, 100),
            kept_index: 0,
            kept: Box::new(track(1, "A", None, 100)),
            same_track: true,
        };
        let json = serde_json::to_value(CleanupChange::from(dup)).unwrap();
        assert_eq!(json["keptIndex"], 0);
        assert_eq!(json["sameTrack"], true);
        assert_eq!(json["op"]["op"], "remove");
        assert_eq!(json["suggested"], true);
        let lost = CleanupItem::Unavailable {
            index: 5,
            track: track(1, "A", None, 100),
            replacement: None,
        };
        let json = serde_json::to_value(CleanupChange::from(lost)).unwrap();
        assert_eq!(
            json["suggested"], false,
            "permanent removal: not ticked by default"
        );
        assert_eq!(json["op"]["op"], "remove");
    }

    #[test]
    fn replacement_by_isrc() {
        let original = track(1, "A", Some("I1"), 200);
        let mut other_album = track(2, "A", Some("I1"), 200);
        other_album.album.title = "Best Of".into();
        let same_album = track(3, "A", Some("I1"), 201);
        let far = track(4, "A", Some("I1"), 260);
        let other_isrc = track(5, "A", Some("I2"), 200);
        let found = pick_replacement(
            &original,
            vec![original.clone(), other_album, far, other_isrc, same_album],
            true,
        );
        assert_eq!(found.map(|t| t.id.0), Some(3));
    }

    #[test]
    fn replacement_by_song() {
        let original = track(1, "A", Some("I1"), 200);
        let mut explicit = track(2, "A (2015 Remaster)", Some("I8"), 200);
        explicit.explicit = true;
        let clean = track(3, "A", Some("I9"), 199);
        let mut other_artist = track(4, "A", Some("I7"), 200);
        other_artist.artists[0].id = 99;
        let found = pick_replacement(&original, vec![explicit, other_artist, clean], false);
        assert_eq!(found.map(|t| t.id.0), Some(3));
        assert_eq!(
            pick_replacement(&original, vec![track(5, "B", None, 200)], false),
            None
        );
    }
}
