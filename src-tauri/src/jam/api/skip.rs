//! Guest vote to skip the current track (#433), if the host allows it.
//!
//! Fixed threshold chosen by the host (`JamRules::skip_votes`): the host does not know
//! how many guests are looking at the page, so a threshold relative to "present" guests
//! would be wrong. One vote per guest (token) and per track; on the local network, one vote
//! per address too (a device opening private tabs votes only once). Through the relay,
//! several guests on shared mobile data share an address: the token alone counts, the
//! per-address limits bound abuse. The vote names the target track: a vote sent just
//! before a track change does not skip the next one.

use std::collections::HashSet;
use std::net::IpAddr;

use serde::{Deserialize, Serialize};

use super::{Backend, Core, backend_error};
use crate::jam::guard::Action;
use crate::jam::http::Response;

/// Votes for the current track, forgotten when the track changes.
#[derive(Debug, Default)]
pub struct SkipVotes {
    track: Option<u64>,
    keys: HashSet<String>,
    ips: HashSet<IpAddr>,
    /// Threshold reached for this track: it is being skipped, no more votes.
    passed: bool,
}

/// Result of a vote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tally {
    pub votes: usize,
    /// This vote reached the threshold: skip the track (only once per track).
    pub pass: bool,
}

impl SkipVotes {
    fn sync(&mut self, track: u64) {
        if self.track != Some(track) {
            *self = Self {
                track: Some(track),
                ..Self::default()
            };
        }
    }

    /// Votes for `track`; those for another track are forgotten.
    pub fn count(&mut self, track: u64) -> usize {
        self.sync(track);
        self.keys.len()
    }

    /// Counts the vote of `key` (and of its address, if `ip`) for `track`. A second vote by
    /// the same guest or address changes nothing.
    pub fn vote(&mut self, track: u64, key: &str, ip: Option<IpAddr>, needed: u8) -> Tally {
        self.sync(track);
        let fresh_ip = ip.is_none_or(|ip| !self.ips.contains(&ip));
        if !self.passed && fresh_ip && self.keys.insert(key.to_owned()) {
            if let Some(ip) = ip {
                self.ips.insert(ip);
            }
            if self.keys.len() >= usize::from(needed) {
                self.passed = true;
                return Tally {
                    votes: self.keys.len(),
                    pass: true,
                };
            }
        }
        Tally {
            votes: self.keys.len(),
            pass: false,
        }
    }
}

/// Vote as seen by guests: count and threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestSkip {
    pub votes: usize,
    pub needed: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkipBody {
    track_id: u64,
}

impl Core {
    /// Vote on the current track for the guests' state, `None` without the rule.
    pub(super) fn skip_view(&self, track: Option<u64>, game: bool) -> Option<GuestSkip> {
        let rules = self.rules();
        if !rules.skip_vote || game {
            return None;
        }
        let track = track?;
        let votes = self.skip_votes.lock().ok()?.count(track);
        Some(GuestSkip {
            votes,
            needed: rules.skip_votes,
        })
    }

    /// `POST api/skip {guest, trackId}`: vote to skip `trackId`, if it is still playing.
    /// The vote that reaches the threshold skips the track.
    pub(super) async fn skip<B: Backend>(
        &self,
        body: &[u8],
        ip: IpAddr,
        key: &str,
        backend: &B,
    ) -> Response {
        let rules = self.rules();
        if !rules.skip_vote {
            return Response::error(403, "skip");
        }
        let Ok(body) = serde_json::from_slice::<SkipBody>(body) else {
            return Response::error(400, "body");
        };
        if !self.allow(ip, key, Action::Vote) {
            return Response::error(429, "slow");
        }
        let snapshot = match backend.state().await {
            Ok(snapshot) => snapshot,
            Err(e) => return backend_error(&e),
        };
        if snapshot.jam_game().is_some() {
            return Response::error(409, "game");
        }
        let now = snapshot.now.as_ref().map(|t| t.id.0);
        if now != Some(body.track_id) {
            return Response::error(409, "changed");
        }
        let ip = self.ban_ips.then_some(ip);
        let tally = match self.skip_votes.lock() {
            Ok(mut votes) => votes.vote(body.track_id, key, ip, rules.skip_votes),
            Err(_) => return Response::error(502, "backend"),
        };
        if tally.pass
            && let Err(e) = backend.skip_by_vote().await
        {
            return backend_error(&e);
        }
        self.refresh.notify_one();
        Response::json(
            200,
            &serde_json::json!({
                "votes": tally.votes,
                "needed": rules.skip_votes,
                "skipped": tally.pass,
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAN: Option<IpAddr> = Some(IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 20)));
    const OTHER: Option<IpAddr> = Some(IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 21)));

    #[test]
    fn the_vote_that_reaches_the_threshold_passes_once() {
        let mut votes = SkipVotes::default();
        assert_eq!(
            votes.vote(7, "a", None, 2),
            Tally {
                votes: 1,
                pass: false
            }
        );
        // Same guest: nothing changes.
        assert_eq!(votes.vote(7, "a", None, 2).votes, 1);
        assert_eq!(
            votes.vote(7, "b", None, 2),
            Tally {
                votes: 2,
                pass: true
            }
        );
        // Threshold already reached: the track is being skipped.
        assert!(!votes.vote(7, "c", None, 2).pass);
    }

    #[test]
    fn votes_are_forgotten_when_the_track_changes() {
        let mut votes = SkipVotes::default();
        votes.vote(7, "a", None, 3);
        votes.vote(7, "b", None, 3);
        assert_eq!(votes.count(7), 2);
        assert_eq!(votes.count(8), 0);
        assert_eq!(votes.vote(8, "a", None, 3).votes, 1);
    }

    #[test]
    fn on_the_local_network_one_device_votes_once() {
        let mut votes = SkipVotes::default();
        votes.vote(7, "onglet-1", LAN, 3);
        // Other token (private browsing), same phone: ignored.
        assert_eq!(votes.vote(7, "onglet-2", LAN, 3).votes, 1);
        assert_eq!(votes.vote(7, "autre", OTHER, 3).votes, 2);
    }
}
