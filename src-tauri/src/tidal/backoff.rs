//! Shared pause after a 429 from TIDAL (#37).
//!
//! Requests run in parallel (paging, new releases, "Save for later"): when TIDAL answers
//! "Too Many Requests", every request waits for the same pause instead of each one
//! retrying on its own. The pause follows `Retry-After` when it gives seconds, otherwise it
//! starts at 2 s and doubles up to 60 s; a success brings it back to 2 s. A request that
//! would wait longer than `MAX_WAIT` fails at once without reaching TIDAL.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// First pause without `Retry-After`, then doubled.
const FIRST: Duration = Duration::from_secs(2);
/// Longest pause without `Retry-After`.
const MAX_DOUBLED: Duration = Duration::from_secs(60);
/// Longest pause taken from `Retry-After`.
const MAX_RETRY_AFTER: Duration = Duration::from_secs(10 * 60);
/// Longest a request waits for the pause before failing.
pub(super) const MAX_WAIT: Duration = Duration::from_secs(30);

#[derive(Debug)]
struct State {
    until: Option<Instant>,
    next: Duration,
}

#[derive(Debug)]
pub(super) struct Backoff(Mutex<State>);

impl Default for Backoff {
    fn default() -> Self {
        Self(Mutex::new(State {
            until: None,
            next: FIRST,
        }))
    }
}

/// What a request does before it is sent.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Gate {
    Go,
    Wait(Duration),
    /// Pause longer than `MAX_WAIT`: fail without calling TIDAL.
    Refuse(Duration),
}

impl Backoff {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        // The state stays consistent whatever panicked while holding the lock.
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(super) fn gate(&self, now: Instant) -> Gate {
        let Some(until) = self.state().until else {
            return Gate::Go;
        };
        match until.checked_duration_since(now) {
            None => Gate::Go,
            Some(left) if left.is_zero() => Gate::Go,
            Some(left) if left > MAX_WAIT => Gate::Refuse(left),
            Some(left) => Gate::Wait(left),
        }
    }

    /// Records a 429; returns the pause now in force.
    pub(super) fn hit(&self, retry_after: Option<Duration>, now: Instant) -> Duration {
        let mut state = self.state();
        let delay = match retry_after {
            Some(d) => d.min(MAX_RETRY_AFTER),
            None => {
                let d = state.next;
                state.next = (d * 2).min(MAX_DOUBLED);
                d
            }
        };
        let until = now + delay;
        // Parallel 429s do not shorten a pause already set.
        if state.until.is_none_or(|u| u < until) {
            state.until = Some(until);
        }
        state.until.map_or(delay, |u| u - now)
    }

    /// A request went through: the next pause starts again from the first step.
    pub(super) fn success(&self) {
        self.state().next = FIRST;
    }
}

/// `Retry-After` in seconds. The HTTP date form is not read (no date parser among the
/// dependencies): the doubled pause applies then.
pub(super) fn parse_retry_after(value: &str) -> Option<Duration> {
    value.trim().parse::<u64>().ok().map(Duration::from_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_pause_lets_requests_go() {
        assert_eq!(Backoff::default().gate(Instant::now()), Gate::Go);
    }

    #[test]
    fn doubles_without_retry_after_then_resets_on_success() {
        let b = Backoff::default();
        let t = Instant::now();
        assert_eq!(b.hit(None, t), Duration::from_secs(2));
        assert_eq!(b.gate(t), Gate::Wait(Duration::from_secs(2)));
        assert_eq!(b.gate(t + Duration::from_secs(2)), Gate::Go);
        let t2 = t + Duration::from_secs(3);
        assert_eq!(b.hit(None, t2), Duration::from_secs(4));
        let t3 = t2 + Duration::from_secs(5);
        assert_eq!(b.hit(None, t3), Duration::from_secs(8));
        b.success();
        let t4 = t3 + Duration::from_secs(9);
        assert_eq!(b.hit(None, t4), Duration::from_secs(2));
    }

    #[test]
    fn doubling_is_capped() {
        let b = Backoff::default();
        let mut t = Instant::now();
        let mut last = Duration::ZERO;
        for _ in 0..10 {
            last = b.hit(None, t);
            t += last + Duration::from_secs(1);
        }
        assert_eq!(last, MAX_DOUBLED);
    }

    #[test]
    fn retry_after_wins_and_is_capped() {
        let b = Backoff::default();
        let t = Instant::now();
        assert_eq!(
            b.hit(Some(Duration::from_secs(7)), t),
            Duration::from_secs(7)
        );
        let b = Backoff::default();
        assert_eq!(b.hit(Some(Duration::from_secs(86_400)), t), MAX_RETRY_AFTER);
    }

    #[test]
    fn a_shorter_429_does_not_shorten_the_pause() {
        let b = Backoff::default();
        let t = Instant::now();
        b.hit(Some(Duration::from_secs(20)), t);
        let t2 = t + Duration::from_secs(1);
        assert_eq!(
            b.hit(Some(Duration::from_secs(1)), t2),
            Duration::from_secs(19)
        );
    }

    #[test]
    fn long_pause_refuses_instead_of_waiting() {
        let b = Backoff::default();
        let t = Instant::now();
        b.hit(Some(Duration::from_secs(120)), t);
        assert_eq!(b.gate(t), Gate::Refuse(Duration::from_secs(120)));
        assert_eq!(
            b.gate(t + Duration::from_secs(100)),
            Gate::Wait(Duration::from_secs(20))
        );
    }

    #[test]
    fn retry_after_seconds_only() {
        assert_eq!(parse_retry_after(" 30 "), Some(Duration::from_secs(30)));
        assert_eq!(parse_retry_after("Wed, 21 Oct 2026 07:28:00 GMT"), None);
        assert_eq!(parse_retry_after("-1"), None);
    }
}
