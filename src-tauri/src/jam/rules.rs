//! The host's rules for the Jam (#288): pure logic, tested without a player or network.
//!
//! The rules are kept from one Jam to the next (`jam.rules` setting); the suspension of
//! additions and the bans stay specific to each Jam (`Core`).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::player::volume::MAX_LEVEL;

/// Largest accepted limit of pending tracks per guest.
pub const MAX_PENDING_LIMIT: u32 = 50;
/// Tracks from the Jam waiting in the manual queue, all guests together (#256): an address
/// that draws tokens on the fly does not fill the host's queue.
pub const MAX_JAM_PENDING: usize = 100;
/// Default cap on the volume set by guests (#449).
pub const DEFAULT_GUEST_VOLUME_MAX: u8 = 80;
/// Votes needed to skip a track (#433): default, bounds.
pub const DEFAULT_SKIP_VOTES: u8 = 3;
pub const MIN_SKIP_VOTES: u8 = 2;
pub const MAX_SKIP_VOTES: u8 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct JamRules {
    /// Guests can pause and resume (#222).
    pub guest_controls: bool,
    /// Pending tracks at most per guest; `None` = no limit.
    pub max_pending: Option<u32>,
    /// Guests can add a whole album (#223).
    pub albums: bool,
    /// Guests can set the volume (#449), off by default.
    pub guest_volume: bool,
    /// Highest volume a guest can ask for (1 to 100).
    pub guest_volume_max: u8,
    /// Guests can vote to skip the current track (#433), off by default.
    pub skip_vote: bool,
    /// Votes that skip the track (2 to 10): a fixed threshold, the host does not know how
    /// many guests are looking at the page.
    pub skip_votes: u8,
}

impl Default for JamRules {
    fn default() -> Self {
        Self {
            guest_controls: true,
            max_pending: None,
            albums: true,
            guest_volume: false,
            guest_volume_max: DEFAULT_GUEST_VOLUME_MAX,
            skip_vote: false,
            skip_votes: DEFAULT_SKIP_VOTES,
        }
    }
}

impl JamRules {
    /// Limit brought between 1 and `MAX_PENDING_LIMIT` (0 = no limit), volume cap between
    /// 1 and 100, vote threshold between 2 and 10.
    pub fn sanitized(self) -> Self {
        Self {
            max_pending: self
                .max_pending
                .filter(|n| *n > 0)
                .map(|n| n.min(MAX_PENDING_LIMIT)),
            guest_volume_max: self.guest_volume_max.clamp(1, MAX_LEVEL),
            skip_votes: self.skip_votes.clamp(MIN_SKIP_VOTES, MAX_SKIP_VOTES),
            ..self
        }
    }

    /// Level applied for a guest's `level` request: brought down to the cap.
    pub fn guest_level(&self, level: u8) -> u8 {
        level.min(self.guest_volume_max)
    }

    /// Room left for a guest who already has `pending` tracks waiting; `None` = no limit.
    pub fn room(&self, pending: usize) -> Option<usize> {
        self.max_pending
            .map(|max| (max as usize).saturating_sub(pending))
    }
}

/// Why an addition is refused; the name is the error type the page receives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// All the tracks are already in the manual queue.
    Duplicate,
    /// The guest has reached their limit of pending tracks.
    Full,
    /// The Jam has reached its share of the manual queue (`MAX_JAM_PENDING`, #256).
    QueueFull,
}

impl Refusal {
    pub fn kind(self) -> (u16, &'static str) {
        match self {
            Self::Duplicate => (409, "duplicate"),
            Self::Full => (429, "full"),
            Self::QueueFull => (429, "queue"),
        }
    }
}

/// Room left for the Jam in the queue when `pending` of its tracks are waiting there.
pub fn jam_room(pending: usize) -> usize {
    MAX_JAM_PENDING.saturating_sub(pending)
}

/// Tracks of an addition to put in the queue: without those already waiting in the manual
/// queue or the repeats, cut to the guest's remaining room (`room`, `None` = no limit) and
/// to the Jam's (`jam_room`).
pub fn admit(
    candidates: &[u64],
    queued: &[u64],
    room: Option<usize>,
    jam_room: usize,
) -> Result<Vec<u64>, Refusal> {
    let mut seen: HashSet<u64> = queued.iter().copied().collect();
    let fresh: Vec<u64> = candidates
        .iter()
        .copied()
        .filter(|id| seen.insert(*id))
        .collect();
    if fresh.is_empty() {
        return Err(Refusal::Duplicate);
    }
    if jam_room == 0 {
        return Err(Refusal::QueueFull);
    }
    match room {
        Some(0) => Err(Refusal::Full),
        room => Ok(fresh
            .into_iter()
            .take(room.unwrap_or(usize::MAX).min(jam_room))
            .collect()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_keep_the_jam_open() {
        let rules = JamRules::default();
        assert!(rules.guest_controls && rules.albums);
        assert_eq!(rules.room(100), None);
        // Setting from before #288 or incomplete: default values.
        let saved: JamRules = serde_json::from_str(r#"{"albums":false}"#).unwrap();
        assert!(saved.guest_controls);
        assert!(!saved.albums);
        // Setting from before #449: volume closed to guests, default cap.
        assert!(!saved.guest_volume);
        assert_eq!(saved.guest_volume_max, DEFAULT_GUEST_VOLUME_MAX);
        // Setting from before #433: vote closed, default threshold.
        assert!(!saved.skip_vote);
        assert_eq!(saved.skip_votes, DEFAULT_SKIP_VOTES);
    }

    #[test]
    fn skip_threshold_is_bounded() {
        let rules = |n| JamRules {
            skip_votes: n,
            ..JamRules::default()
        };
        assert_eq!(rules(0).sanitized().skip_votes, MIN_SKIP_VOTES);
        assert_eq!(rules(1).sanitized().skip_votes, MIN_SKIP_VOTES);
        assert_eq!(rules(5).sanitized().skip_votes, 5);
        assert_eq!(rules(200).sanitized().skip_votes, MAX_SKIP_VOTES);
    }

    #[test]
    fn guest_volume_is_capped() {
        let rules = |max| JamRules {
            guest_volume: true,
            guest_volume_max: max,
            ..JamRules::default()
        };
        assert_eq!(rules(0).sanitized().guest_volume_max, 1);
        assert_eq!(rules(250).sanitized().guest_volume_max, MAX_LEVEL);
        assert_eq!(rules(80).guest_level(95), 80);
        assert_eq!(rules(80).guest_level(30), 30);
    }

    #[test]
    fn limits_are_bounded() {
        let rules = |max| JamRules {
            max_pending: max,
            ..JamRules::default()
        };
        assert_eq!(rules(Some(0)).sanitized().max_pending, None);
        assert_eq!(
            rules(Some(999)).sanitized().max_pending,
            Some(MAX_PENDING_LIMIT)
        );
        assert_eq!(rules(Some(3)).room(1), Some(2));
        assert_eq!(rules(Some(3)).room(5), Some(0));
    }

    #[test]
    fn duplicates_are_skipped_and_the_rest_fits_the_room() {
        let all = MAX_JAM_PENDING;
        assert_eq!(admit(&[1], &[], None, all), Ok(vec![1]));
        assert_eq!(admit(&[1], &[1, 2], None, all), Err(Refusal::Duplicate));
        assert_eq!(admit(&[1, 2, 3, 2, 4], &[2], None, all), Ok(vec![1, 3, 4]));
        assert_eq!(admit(&[1, 2, 3], &[], Some(2), all), Ok(vec![1, 2]));
        assert_eq!(admit(&[1], &[], Some(0), all), Err(Refusal::Full));
        // Already waiting: the duplicate wins over the limit (more accurate message).
        assert_eq!(admit(&[1], &[1], Some(0), all), Err(Refusal::Duplicate));
    }

    #[test]
    fn the_jam_share_of_the_queue_is_capped() {
        assert_eq!(jam_room(MAX_JAM_PENDING - 2), 2);
        assert_eq!(jam_room(MAX_JAM_PENDING + 5), 0);
        // An album is cut to the smaller of the two rooms.
        assert_eq!(admit(&[1, 2, 3, 4], &[], None, 2), Ok(vec![1, 2]));
        assert_eq!(admit(&[1, 2, 3, 4], &[], Some(1), 2), Ok(vec![1]));
        // The full queue wins over the guest's limit: it is not their doing.
        assert_eq!(admit(&[1], &[], Some(0), 0), Err(Refusal::QueueFull));
    }
}
