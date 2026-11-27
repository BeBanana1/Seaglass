//! Guest adds, for the duration of a Jam (#25): displayed name, author and batch of each track.
//!
//! Kept here, in memory, and not in the player's queue: the queue stays the host's
//! (persisted, with no notion of guest), and the Jam stops with the app. The host's UI and the
//! guests' page show the name next to the tracks of the manual queue.
//!
//! A **batch** is one add: a track, or the tracks of an album (#223). The author is the
//! token drawn by the guest's browser: only they can remove their batches. The first name
//! is not an identity (anyone can type the same one).

use std::collections::{HashMap, VecDeque};

use super::members::Members;

/// Maximum length of a name, in characters.
pub const MAX_NAME_CHARS: usize = 24;
/// Tracks kept: beyond that, the oldest are forgotten.
const MAX_ENTRIES: usize = 500;
/// Guest token: hexadecimal, 16 to 64 characters.
const TOKEN_MIN: usize = 16;
const TOKEN_MAX: usize = 64;

/// Name entered by a guest: spaces collapsed, control characters removed, cut to
/// `MAX_NAME_CHARS`. Empty = no name.
pub fn sanitize_name(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_NAME_CHARS)
        .collect();
    let cleaned = cleaned.trim_end().to_owned();
    (!cleaned.is_empty()).then_some(cleaned)
}

/// Token of a guest browser, `None` if it has the wrong shape (old page:
/// the add stays allowed, with no removal possible).
pub fn sanitize_token(raw: &str) -> Option<String> {
    let ok =
        (TOKEN_MIN..=TOKEN_MAX).contains(&raw.len()) && raw.bytes().all(|b| b.is_ascii_hexdigit());
    ok.then(|| raw.to_ascii_lowercase())
}

#[derive(Debug, Clone, PartialEq)]
struct Added {
    name: Option<String>,
    owner: Option<String>,
    batch: u32,
}

#[derive(Debug, Default)]
pub struct Guests {
    by_track: HashMap<u64, Added>,
    order: VecDeque<u64>,
    next_batch: u32,
    /// Guests who added, and those the host excluded (#288).
    pub members: Members,
}

impl Guests {
    /// New batch; its tracks are noted afterwards by `note`.
    pub fn new_batch(&mut self) -> u32 {
        self.next_batch = self.next_batch.wrapping_add(1);
        self.next_batch
    }

    /// The last guest to add a track becomes its author.
    pub fn note(&mut self, track: u64, batch: u32, name: Option<String>, owner: Option<String>) {
        let added = Added { name, owner, batch };
        if self.by_track.insert(track, added).is_none() {
            self.order.push_back(track);
        }
        while self.order.len() > MAX_ENTRIES {
            if let Some(old) = self.order.pop_front() {
                self.by_track.remove(&old);
            }
        }
    }

    pub fn name(&self, track: u64) -> Option<&str> {
        self.by_track.get(&track)?.name.as_deref()
    }

    pub fn batch(&self, track: u64) -> Option<u32> {
        self.by_track.get(&track).map(|a| a.batch)
    }

    /// Removes the tracks of a batch of this author (just one if `track` is given) and returns
    /// their ids. A track added again by someone else is no longer theirs.
    pub fn take(&mut self, owner: &str, batch: u32, track: Option<u64>) -> Vec<u64> {
        let taken: Vec<u64> = self
            .order
            .iter()
            .copied()
            .filter(|id| track.is_none_or(|t| t == *id))
            .filter(|id| {
                self.by_track
                    .get(id)
                    .is_some_and(|a| a.batch == batch && a.owner.as_deref() == Some(owner))
            })
            .collect();
        for id in &taken {
            self.by_track.remove(id);
        }
        self.order.retain(|id| !taken.contains(id));
        taken
    }

    /// Tracks of this guest still waiting in the manual queue (#288).
    pub fn pending(&self, owner: &str, manual: &[u64]) -> usize {
        manual
            .iter()
            .filter(|id| {
                self.by_track
                    .get(id)
                    .is_some_and(|a| a.owner.as_deref() == Some(owner))
            })
            .count()
    }

    /// Tracks from the Jam still waiting in the manual queue, all guests
    /// together (#256).
    pub fn pending_total(&self, manual: &[u64]) -> usize {
        manual
            .iter()
            .filter(|id| self.by_track.contains_key(id))
            .count()
    }

    /// Removes all tracks of this guest, whatever the batch (exclusion, #288).
    pub fn take_all(&mut self, owner: &str) -> Vec<u64> {
        let taken: Vec<u64> = self
            .order
            .iter()
            .copied()
            .filter(|id| {
                self.by_track
                    .get(id)
                    .is_some_and(|a| a.owner.as_deref() == Some(owner))
            })
            .collect();
        for id in &taken {
            self.by_track.remove(id);
        }
        self.order.retain(|id| !taken.contains(id));
        taken
    }

    /// Name of the guest for each track, for the host's UI.
    pub fn snapshot(&self) -> HashMap<u64, String> {
        self.by_track
            .iter()
            .filter_map(|(id, a)| Some((*id, a.name.clone()?)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_cleaned_and_bounded() {
        assert_eq!(sanitize_name("  Léa  "), Some("Léa".into()));
        assert_eq!(sanitize_name("Jean\n\tLuc"), Some("Jean Luc".into()));
        assert_eq!(sanitize_name(" \u{7}  "), None);
        let long = sanitize_name(&"é".repeat(40)).unwrap();
        assert_eq!(long.chars().count(), MAX_NAME_CHARS);
        // Cut right after a space: no trailing space.
        let cut = sanitize_name(&format!("{} b", "a".repeat(MAX_NAME_CHARS - 1))).unwrap();
        assert_eq!(cut, "a".repeat(MAX_NAME_CHARS - 1));
    }

    #[test]
    fn tokens_are_hex_of_bounded_length() {
        assert_eq!(
            sanitize_token("0123456789ABCDEF"),
            Some("0123456789abcdef".into())
        );
        assert_eq!(sanitize_token("0123456789abcde"), None);
        assert_eq!(sanitize_token(&"a".repeat(65)), None);
        assert_eq!(sanitize_token("0123456789abcdeg"), None);
    }

    #[test]
    fn last_guest_wins_and_entries_are_bounded() {
        let mut guests = Guests::default();
        guests.note(1, 1, Some("Léa".into()), None);
        guests.note(1, 2, Some("Max".into()), None);
        assert_eq!(guests.name(1), Some("Max"));
        assert_eq!(guests.batch(1), Some(2));
        guests.note(2, 3, None, None);
        assert!(!guests.snapshot().contains_key(&2));
        for id in 3..=(MAX_ENTRIES as u64 + 1) {
            guests.note(id, 4, Some("x".into()), None);
        }
        assert_eq!(guests.name(1), None);
        assert_eq!(guests.by_track.len(), MAX_ENTRIES);
    }

    #[test]
    fn only_the_owner_takes_a_batch() {
        let (lea, max) = ("a".repeat(32), "b".repeat(32));
        let mut guests = Guests::default();
        let album = guests.new_batch();
        for id in [1, 2, 3] {
            guests.note(id, album, Some("Léa".into()), Some(lea.clone()));
        }
        // Max adds track 3 again: it is theirs.
        let single = guests.new_batch();
        guests.note(3, single, Some("Max".into()), Some(max.clone()));

        assert!(guests.take(&max, album, None).is_empty());
        assert_eq!(guests.take(&lea, album, Some(2)), vec![2]);
        assert_eq!(guests.take(&lea, album, None), vec![1]);
        assert_eq!(guests.name(3), Some("Max"));
        assert_eq!(guests.take(&max, single, None), vec![3]);
        assert!(guests.snapshot().is_empty());
    }

    #[test]
    fn pending_counts_only_queued_tracks_of_the_owner() {
        let (lea, max) = ("a".repeat(32), "b".repeat(32));
        let mut guests = Guests::default();
        for id in [1, 2, 3] {
            let batch = guests.new_batch();
            guests.note(id, batch, None, Some(lea.clone()));
        }
        let batch = guests.new_batch();
        guests.note(4, batch, None, Some(max.clone()));
        // Track 1 was played: it is no longer in the manual queue.
        assert_eq!(guests.pending(&lea, &[2, 3, 4]), 2);
        assert_eq!(guests.pending(&max, &[2, 3, 4]), 1);
        assert_eq!(guests.pending_total(&[2, 3, 4, 9]), 3);
        let mut taken = guests.take_all(&lea);
        taken.sort_unstable();
        assert_eq!(taken, vec![1, 2, 3]);
        assert_eq!(guests.pending(&lea, &[2, 3, 4]), 0);
        assert_eq!(guests.batch(4), Some(batch));
    }
}
