//! Access code and per-guest and per-address limits: pure logic, clock passed as an
//! argument.
//!
//! On mobile data, several guests often exit through the same address (#256): each
//! action is thus counted per guest (browser token, otherwise the address) at the
//! thresholds below, and per address with a threshold `IP_FACTOR` times wider. The token is
//! not a strong identity (a browser can draw another whenever it wants): the per-address
//! cap remains the real bound.

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::IpAddr;
use std::time::{Duration, Instant};

use rand::Rng;

/// Without 0/O, 1/I: the code can be read aloud or on a screen from afar.
const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
pub const CODE_LEN: usize = 6;
/// Failed attempts per address over `FAILURE_WINDOW` before the address is blocked; the
/// block lifts when the oldest attempts leave the window (#256).
pub const MAX_FAILURES: usize = 10;
pub const FAILURE_WINDOW: Duration = Duration::from_secs(15 * 60);
pub const ADDS_PER_MINUTE: usize = 10;
/// Enough for search while typing (one request per typing pause).
pub const SEARCHES_PER_MINUTE: usize = 60;
/// Play / pause (#222): enough for normal use, not to turn it into a game.
pub const CONTROLS_PER_MINUTE: usize = 6;
/// Removals of one's own adds (#223).
pub const REMOVES_PER_MINUTE: usize = 30;
/// Thresholds of an address = thresholds of a guest × this factor (#256).
pub const IP_FACTOR: usize = 5;
/// Addresses and guests tracked at most; inactive ones are forgotten when the table is full.
const MAX_ADDRESSES: usize = 1024;
const MAX_GUESTS: usize = 4096;
/// Tracks offered by Jam searches, the only ones guests can add.
const MAX_OFFERED: usize = 5000;
const WINDOW: Duration = Duration::from_secs(60);

/// Owner commands per minute (remote control, #336): a volume or position slider
/// sends one per gesture, never a burst.
pub const COMMANDS_PER_MINUTE: usize = 120;
/// Blind test answers per minute (#441): one per round is plenty.
pub const ANSWERS_PER_MINUTE: usize = 30;
/// Volume set by a guest (#449): one send per slider gesture, counted apart from
/// pause so that one does not block the other.
pub const VOLUMES_PER_MINUTE: usize = 30;
/// Votes to skip a track (#433): one per track is enough, the margin covers refusals
/// (track changed in the meantime).
pub const VOTES_PER_MINUTE: usize = 20;

/// 32⁶ ≈ 1.07 billion codes.
pub fn new_code() -> String {
    code_of_len(CODE_LEN)
}

/// Code of `len` characters from the unambiguous alphabet (5 bits each).
pub fn code_of_len(len: usize) -> String {
    let mut rng = rand::rng();
    (0..len)
        .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
        .collect()
}

/// True if `code` only has characters from the alphabet (code read back from the settings).
pub fn is_code(code: &str, len: usize) -> bool {
    code.len() == len && code.bytes().all(|b| ALPHABET.contains(&b))
}

/// Compares an entered code (case, spaces and dashes ignored) in constant time.
pub fn code_matches(expected: &str, given: &str) -> bool {
    let given: Vec<u8> = given
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'-')
        .map(|b| b.to_ascii_uppercase())
        .collect();
    let expected = expected.as_bytes();
    if given.len() != expected.len() {
        return false;
    }
    expected
        .iter()
        .zip(&given)
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Search,
    Add,
    Control,
    Remove,
    /// Owner command (remote control, #336).
    Command,
    /// Blind test answer (#441).
    Answer,
    /// Volume set by a guest (#449).
    Volume,
    /// Vote to skip the track (#433).
    Vote,
}

impl Action {
    fn per_minute(self) -> usize {
        match self {
            Self::Search => SEARCHES_PER_MINUTE,
            Self::Add => ADDS_PER_MINUTE,
            Self::Control => CONTROLS_PER_MINUTE,
            Self::Remove => REMOVES_PER_MINUTE,
            Self::Command => COMMANDS_PER_MINUTE,
            Self::Answer => ANSWERS_PER_MINUTE,
            Self::Volume => VOLUMES_PER_MINUTE,
            Self::Vote => VOTES_PER_MINUTE,
        }
    }
}

/// Recent actions of a guest or an address.
#[derive(Default)]
struct Counters {
    searches: VecDeque<Instant>,
    adds: VecDeque<Instant>,
    controls: VecDeque<Instant>,
    removes: VecDeque<Instant>,
    commands: VecDeque<Instant>,
    answers: VecDeque<Instant>,
    volumes: VecDeque<Instant>,
    votes: VecDeque<Instant>,
}

impl Counters {
    fn times(&mut self, action: Action) -> &mut VecDeque<Instant> {
        match action {
            Action::Search => &mut self.searches,
            Action::Add => &mut self.adds,
            Action::Control => &mut self.controls,
            Action::Remove => &mut self.removes,
            Action::Command => &mut self.commands,
            Action::Answer => &mut self.answers,
            Action::Volume => &mut self.volumes,
            Action::Vote => &mut self.votes,
        }
    }

    /// Room for one more action under `max` (old actions forgotten along the way).
    fn has_room(&mut self, action: Action, max: usize, now: Instant) -> bool {
        let times = self.times(action);
        while times
            .front()
            .is_some_and(|t| now.duration_since(*t) >= WINDOW)
        {
            times.pop_front();
        }
        times.len() < max
    }

    fn idle(&self, now: Instant) -> bool {
        [
            &self.searches,
            &self.adds,
            &self.controls,
            &self.removes,
            &self.commands,
            &self.answers,
            &self.volumes,
            &self.votes,
        ]
        .iter()
        .all(|q| q.back().is_none_or(|t| now.duration_since(*t) >= WINDOW))
    }
}

#[derive(Default)]
struct Address {
    failures: VecDeque<Instant>,
    counters: Counters,
}

impl Address {
    fn prune_failures(&mut self, now: Instant) {
        while self
            .failures
            .front()
            .is_some_and(|t| now.duration_since(*t) >= FAILURE_WINDOW)
        {
            self.failures.pop_front();
        }
    }

    fn idle(&mut self, now: Instant) -> bool {
        self.prune_failures(now);
        self.failures.is_empty() && self.counters.idle(now)
    }
}

#[derive(Default)]
pub struct Limits {
    addresses: HashMap<IpAddr, Address>,
    guests: HashMap<String, Counters>,
}

impl Limits {
    /// Address blocked (too many recent failed attempts) or refused (too many active addresses).
    pub fn blocked(&mut self, ip: IpAddr, now: Instant) -> bool {
        match self.address(ip, now) {
            Some(a) => {
                a.prune_failures(now);
                a.failures.len() >= MAX_FAILURES
            }
            None => true,
        }
    }

    pub fn fail(&mut self, ip: IpAddr, now: Instant) {
        if let Some(a) = self.address(ip, now) {
            a.failures.push_back(now);
        }
    }

    /// True if the action is allowed now for guest `guest` (their key, `owner_key`)
    /// from `ip`; it is then counted for both.
    pub fn allow(&mut self, ip: IpAddr, guest: &str, action: Action, now: Instant) -> bool {
        let max = action.per_minute();
        let ip_room = match self.address(ip, now) {
            Some(a) => a.counters.has_room(action, max * IP_FACTOR, now),
            None => false,
        };
        if !ip_room {
            return false;
        }
        let Some(counters) = self.guest(guest, now) else {
            return false;
        };
        if !counters.has_room(action, max, now) {
            return false;
        }
        counters.times(action).push_back(now);
        if let Some(a) = self.addresses.get_mut(&ip) {
            a.counters.times(action).push_back(now);
        }
        true
    }

    fn address(&mut self, ip: IpAddr, now: Instant) -> Option<&mut Address> {
        if !self.addresses.contains_key(&ip) && self.addresses.len() >= MAX_ADDRESSES {
            self.addresses.retain(|_, a| !a.idle(now));
            if self.addresses.len() >= MAX_ADDRESSES {
                return None;
            }
        }
        Some(self.addresses.entry(ip).or_default())
    }

    fn guest(&mut self, key: &str, now: Instant) -> Option<&mut Counters> {
        if !self.guests.contains_key(key) && self.guests.len() >= MAX_GUESTS {
            self.guests.retain(|_, c| !c.idle(now));
            if self.guests.len() >= MAX_GUESTS {
                return None;
            }
        }
        Some(self.guests.entry(key.to_owned()).or_default())
    }
}

/// Tracks coming out of Jam searches: a guest can only add these, not
/// any identifier.
#[derive(Default)]
pub struct Offered {
    ids: HashSet<u64>,
    order: VecDeque<u64>,
}

impl Offered {
    pub fn note(&mut self, ids: impl IntoIterator<Item = u64>) {
        for id in ids {
            if self.ids.insert(id) {
                self.order.push_back(id);
            }
        }
        while self.order.len() > MAX_OFFERED {
            if let Some(old) = self.order.pop_front() {
                self.ids.remove(&old);
            }
        }
    }

    pub fn contains(&self, id: u64) -> bool {
        self.ids.contains(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20));
    const OTHER: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 21));

    #[test]
    fn codes_use_the_unambiguous_alphabet() {
        let code = new_code();
        assert_eq!(code.len(), CODE_LEN);
        assert!(code.bytes().all(|b| ALPHABET.contains(&b)));
        assert!(!code.contains('O') && !code.contains('0'));
    }

    #[test]
    fn code_comparison_ignores_case_spaces_and_dashes() {
        assert!(code_matches("AB23CD", "ab2-3cd"));
        assert!(code_matches("AB23CD", " AB 23 CD "));
        assert!(!code_matches("AB23CD", "AB23CE"));
        assert!(!code_matches("AB23CD", "AB23C"));
        assert!(!code_matches("AB23CD", ""));
    }

    #[test]
    fn failures_block_the_address_for_a_while() {
        let mut limits = Limits::default();
        let t0 = Instant::now();
        for i in 0..MAX_FAILURES - 1 {
            limits.fail(IP, t0 + Duration::from_secs(i as u64));
        }
        assert!(!limits.blocked(IP, t0));
        limits.fail(IP, t0 + Duration::from_secs(60));
        assert!(limits.blocked(IP, t0 + Duration::from_secs(60)));
        assert!(!limits.blocked(OTHER, t0));
        // The first attempt leaves the window: the address can retry.
        assert!(limits.blocked(IP, t0 + FAILURE_WINDOW - Duration::from_secs(1)));
        assert!(!limits.blocked(IP, t0 + FAILURE_WINDOW));
    }

    #[test]
    fn adds_are_limited_per_guest_and_minute() {
        let mut limits = Limits::default();
        let t0 = Instant::now();
        for _ in 0..ADDS_PER_MINUTE {
            assert!(limits.allow(IP, "lea", Action::Add, t0));
        }
        assert!(!limits.allow(IP, "lea", Action::Add, t0 + Duration::from_secs(30)));
        // Another guest behind the same address has their own adds (#256).
        assert!(limits.allow(IP, "max", Action::Add, t0));
        // Searches and playback commands have their own counter.
        assert!(limits.allow(IP, "lea", Action::Search, t0));
        assert!(limits.allow(IP, "lea", Action::Control, t0));
        assert!(limits.allow(IP, "lea", Action::Add, t0 + WINDOW));
    }

    #[test]
    fn an_address_keeps_a_wider_ceiling() {
        let mut limits = Limits::default();
        let t0 = Instant::now();
        // Fresh tokens on every add do not exceed the address's cap.
        for i in 0..ADDS_PER_MINUTE * IP_FACTOR {
            assert!(limits.allow(IP, &format!("k{i}"), Action::Add, t0));
        }
        assert!(!limits.allow(IP, "fresh", Action::Add, t0));
        assert!(limits.allow(OTHER, "fresh", Action::Add, t0));
        // A refusal by the address does not count for the guest.
        assert!(limits.allow(IP, "fresh", Action::Add, t0 + WINDOW));
    }

    #[test]
    fn idle_guests_are_forgotten_when_the_table_is_full() {
        let mut limits = Limits::default();
        let t0 = Instant::now();
        for i in 0..MAX_GUESTS {
            limits.guests.insert(format!("k{i}"), Counters::default());
        }
        limits.guests.get_mut("k0").unwrap().adds.push_back(t0);
        assert!(limits.allow(IP, "new", Action::Search, t0 + Duration::from_secs(1)));
        assert!(limits.guests.contains_key("k0"));
        assert_eq!(limits.guests.len(), 2);
    }

    #[test]
    fn offered_ids_are_bounded() {
        let mut offered = Offered::default();
        offered.note(0..(MAX_OFFERED as u64 + 10));
        assert!(!offered.contains(0));
        assert!(offered.contains(MAX_OFFERED as u64 + 9));
        offered.note([5]);
        assert_eq!(offered.order.len(), MAX_OFFERED);
    }
}
