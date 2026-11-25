//! Playback handoff between two devices (#414): the queue, the position and the
//! metadata of what is shown. Never a stream URL (signed, they expire): the
//! device that takes over requests its own.

use serde::{Deserialize, Serialize};

use super::actor::Player;
use super::queue::Queue;
use crate::tidal::models::Track;

/// Upcoming context tracks whose metadata travels (the UI shows 50).
const UPCOMING_TRACKS: usize = 100;
/// Most metadata accepted: beyond, the rest is ignored.
pub const MAX_TRACKS: usize = 2_500;

/// What goes from one device to the other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Handoff {
    pub queue: Queue,
    /// Position in the current track, in seconds.
    pub position: f64,
    /// Metadata of the current track, the manual queue and the start of the rest, for an
    /// immediate display; the others are read when they are played.
    pub tracks: Vec<Track>,
}

impl Handoff {
    /// The handoff if it is consistent (queue checked, position bounded), otherwise `None`.
    pub fn received(self, is_uuid: impl Fn(&str) -> bool) -> Option<Self> {
        let queue = self.queue.received(is_uuid)?;
        let position = if self.position.is_finite() {
            self.position.max(0.0)
        } else {
            0.0
        };
        let mut tracks = self.tracks;
        tracks.truncate(MAX_TRACKS);
        Some(Self {
            queue,
            position,
            tracks,
        })
    }
}

impl Player {
    /// Snapshot of the playback, then pause: TIDAL limits simultaneous playback per
    /// account, the device that takes over must not play at the same time. `None` if
    /// nothing is playing.
    pub(super) async fn take_handoff(&mut self) -> Option<Handoff> {
        let current = self.queue.current()?;
        let mut ids = vec![current];
        ids.extend(self.queue.manual());
        ids.extend(self.queue.upcoming_context(UPCOMING_TRACKS));
        let handoff = Handoff {
            queue: self.queue.clone(),
            position: self.state.time_pos,
            tracks: self.tidal.cache.get_many(ids),
        };
        self.set_paused(true).await;
        // The other device goes on: this listen ends here (#34).
        self.log_close();
        tracing::info!(
            "playback handed off: track {} at {:.0} s",
            current.0,
            handoff.position
        );
        Some(handoff)
    }

    /// Takes over a handed-off playback: its queue replaces this one, then the current track
    /// is played at the received position. `handoff` is already checked (`Handoff::received`).
    pub(super) async fn apply_handoff(&mut self, handoff: Handoff) -> bool {
        let Some(id) = handoff.queue.current() else {
            return false;
        };
        self.tidal.cache.insert_all(&handoff.tracks);
        self.queue = handoff.queue;
        self.on_queue_changed().await;
        tracing::info!(
            "playback taken over from another device: track {} at {:.0} s",
            id.0,
            handoff.position
        );
        self.load_current(id, handoff.position);
        true
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    use super::*;
    use crate::tidal::models::TrackId;

    fn handoff(position: f64) -> Handoff {
        let mut q = Queue::default();
        let ids = (1..4).map(TrackId).collect();
        q.set_context(ids, 1, None, &mut StdRng::seed_from_u64(3));
        Handoff {
            queue: q,
            position,
            tracks: Vec::new(),
        }
    }

    #[test]
    fn position_is_kept_or_clamped() {
        let ok = |p: f64| handoff(p).received(|_| true).map(|h| h.position);
        assert_eq!(ok(83.5), Some(83.5));
        assert_eq!(ok(-4.0), Some(0.0));
        assert_eq!(ok(f64::NAN), Some(0.0));
        assert_eq!(ok(f64::INFINITY), Some(0.0));
    }

    #[test]
    fn an_empty_queue_is_refused() {
        let empty = Handoff {
            queue: Queue::default(),
            position: 0.0,
            tracks: Vec::new(),
        };
        assert_eq!(empty.received(|_| true), None);
    }

    #[test]
    fn the_json_round_trip_keeps_everything() {
        let h = handoff(12.0);
        let json = serde_json::to_string(&h).unwrap();
        let back: Handoff = serde_json::from_str(&json).unwrap();
        assert_eq!(back, h);
    }
}
