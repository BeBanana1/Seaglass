//! When a listen counts for the local history (#202): Last.fm's rule, the track actually
//! played half its length or 4 minutes, and lasts more than 30 s. Time is accumulated from
//! the positions received from the engine: a jump forward does not count, neither does a
//! pause. Pure logic, no clock. Also says when to announce the start of a track to
//! ListenBrainz (#431): once per listen, as soon as it plays.

use crate::tidal::models::TrackId;

/// Shorter tracks: never counted.
const MIN_DURATION: f64 = 30.0;
/// Absolute threshold, for long tracks.
const MAX_THRESHOLD: f64 = 240.0;
/// Maximum gap between two positions for continuous playback: beyond it, it is a jump.
/// Android only sends the position every 5 s with no visible activity (#147).
const MAX_STEP: f64 = 6.0;
/// Back to the start (repeat One, "Previous" after 3 s): new listen.
const RESTART_BELOW: f64 = 5.0;

#[derive(Debug, Default)]
pub struct ListenTracker {
    track: Option<TrackId>,
    last_pos: f64,
    listened: f64,
    counted: bool,
    announced: bool,
}

impl ListenTracker {
    /// New position of the current track (`duration` in seconds, 0 if unknown). Returns the
    /// track once per listen, when the threshold is crossed.
    pub fn on_position(&mut self, track: TrackId, duration: f64, pos: f64) -> Option<TrackId> {
        if self.track != Some(track) {
            *self = Self {
                track: Some(track),
                last_pos: pos,
                ..Self::default()
            };
            return None;
        }
        let step = pos - self.last_pos;
        if step < 0.0 && pos < RESTART_BELOW && self.last_pos - pos > RESTART_BELOW {
            self.listened = 0.0;
            self.counted = false;
            self.announced = false;
        } else if step > 0.0 && step <= MAX_STEP {
            self.listened += step;
        }
        self.last_pos = pos;
        if self.counted || duration <= MIN_DURATION {
            return None;
        }
        if self.listened >= (duration / 2.0).min(MAX_THRESHOLD) {
            self.counted = true;
            return Some(track);
        }
        None
    }

    /// True once per listen of the current track (after `on_position`): it is playing, its
    /// start can be announced.
    pub fn announce(&mut self, track: TrackId) -> bool {
        if self.track != Some(track) || self.announced {
            return false;
        }
        self.announced = true;
        true
    }

    /// Time actually listened of the current track, in seconds.
    pub fn listened(&self) -> f64 {
        self.listened
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: TrackId = TrackId(1);

    /// Positions from 0 to `to` in steps of `step`, returns the number of listens counted.
    fn play(tracker: &mut ListenTracker, duration: f64, from: f64, to: f64, step: f64) -> usize {
        let mut counted = 0;
        let mut pos = from;
        while pos <= to {
            counted += usize::from(tracker.on_position(T, duration, pos).is_some());
            pos += step;
        }
        counted
    }

    #[test]
    fn half_of_the_track() {
        let mut tracker = ListenTracker::default();
        assert_eq!(play(&mut tracker, 200.0, 0.0, 99.0, 0.25), 0);
        assert_eq!(play(&mut tracker, 200.0, 99.25, 200.0, 0.25), 1);
    }

    #[test]
    fn four_minutes_for_long_tracks() {
        let mut tracker = ListenTracker::default();
        assert_eq!(play(&mut tracker, 1200.0, 0.0, 239.0, 1.0), 0);
        assert_eq!(play(&mut tracker, 1200.0, 240.0, 600.0, 1.0), 1);
    }

    #[test]
    fn short_tracks_and_seeks_do_not_count() {
        let mut tracker = ListenTracker::default();
        assert_eq!(play(&mut tracker, 30.0, 0.0, 30.0, 0.25), 0);
        let mut tracker = ListenTracker::default();
        tracker.on_position(T, 200.0, 0.0);
        // Jump to the end: no listen.
        assert_eq!(play(&mut tracker, 200.0, 150.0, 199.0, 1.0), 0);
    }

    #[test]
    fn slow_position_updates_still_count() {
        // Android in the background: one position every 5 s.
        let mut tracker = ListenTracker::default();
        assert_eq!(play(&mut tracker, 200.0, 0.0, 200.0, 5.0), 1);
    }

    #[test]
    fn repeat_one_counts_again() {
        let mut tracker = ListenTracker::default();
        assert_eq!(play(&mut tracker, 100.0, 0.0, 100.0, 1.0), 1);
        assert_eq!(play(&mut tracker, 100.0, 0.0, 100.0, 1.0), 1);
    }

    #[test]
    fn announced_once_per_listen() {
        let mut tracker = ListenTracker::default();
        assert!(!tracker.announce(T));
        tracker.on_position(T, 100.0, 0.0);
        assert!(tracker.announce(T));
        assert!(!tracker.announce(T));
        play(&mut tracker, 100.0, 1.0, 100.0, 1.0);
        assert!(!tracker.announce(T));
        // Repeat One: new listen, new announcement.
        tracker.on_position(T, 100.0, 0.0);
        assert!(tracker.announce(T));
        assert!(!tracker.announce(TrackId(2)));
    }

    #[test]
    fn a_new_track_resets() {
        let mut tracker = ListenTracker::default();
        play(&mut tracker, 200.0, 0.0, 90.0, 1.0);
        assert_eq!(tracker.on_position(TrackId(2), 200.0, 0.0), None);
        // The first track's listen does not carry over to the second.
        assert_eq!(tracker.on_position(TrackId(2), 200.0, 15.0), None);
    }
}
