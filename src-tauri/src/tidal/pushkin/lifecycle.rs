//! When Pushkin's connection is wanted, and which `USER_ACTION` is due (#35).

use std::time::Duration;

use tokio::time::Instant;

use super::Playback;

/// How long the connection stays open after playback stops (a pause, a track loading).
const LINGER: Duration = Duration::from_secs(120);
/// A notification this soon after our own `USER_ACTION` may come from an earlier action of
/// the other client, still in flight: it is ignored.
const CLAIM_GRACE: Duration = Duration::from_millis(1500);

/// When the connection is wanted, and which `USER_ACTION` is due. Pure: the task feeds it
/// the player's messages and the time.
#[derive(Debug, Default)]
pub(super) struct Lifecycle {
    playing: bool,
    /// When playback stopped, while the connection lingers.
    stopped_at: Option<Instant>,
    /// A `USER_ACTION` to send once connected.
    claim: bool,
    claimed_at: Option<Instant>,
    /// No session: nothing to connect with until the player says something new.
    gave_up: bool,
}

impl Lifecycle {
    pub(super) fn on(&mut self, playback: Playback, now: Instant) {
        self.gave_up = false;
        match playback {
            Playback::Claim => {
                self.playing = true;
                self.stopped_at = None;
                self.claim = true;
            }
            Playback::Playing => {
                self.playing = true;
                self.stopped_at = None;
            }
            Playback::Stopped => {
                if self.playing {
                    self.stopped_at = Some(now);
                }
                self.playing = false;
                // Not sent yet: nothing plays here anymore, the other client may go on.
                self.claim = false;
            }
        }
    }

    pub(super) fn wanted(&self, now: Instant) -> bool {
        !self.gave_up && (self.playing || self.lingers_until().is_some_and(|end| now < end))
    }

    /// End of the lingering after a stop.
    pub(super) fn lingers_until(&self) -> Option<Instant> {
        (!self.playing)
            .then_some(self.stopped_at?)
            .map(|at| at + LINGER)
    }

    /// The `USER_ACTION` due now, if any (marked as sent).
    pub(super) fn take_claim(&mut self, now: Instant) -> bool {
        let due = std::mem::take(&mut self.claim);
        if due {
            self.claimed_at = Some(now);
        }
        due
    }

    /// The `USER_ACTION` could not go out: it is sent on the next connection.
    pub(super) fn keep_claim(&mut self) {
        self.claim = self.playing;
    }

    pub(super) fn ignores_notification(&self, now: Instant) -> bool {
        self.claimed_at
            .is_some_and(|at| now.saturating_duration_since(at) < CLAIM_GRACE)
    }

    pub(super) fn give_up(&mut self) {
        self.gave_up = true;
        self.claim = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_connection_follows_playback_and_lingers() {
        let t0 = Instant::now();
        let mut life = Lifecycle::default();
        assert!(!life.wanted(t0), "idle at startup");
        life.on(Playback::Playing, t0);
        assert!(life.wanted(t0));
        life.on(Playback::Stopped, t0);
        assert!(life.wanted(t0 + LINGER - Duration::from_secs(1)));
        assert!(!life.wanted(t0 + LINGER));
        // A stop while nothing played does not open a lingering window.
        let mut idle = Lifecycle::default();
        idle.on(Playback::Stopped, t0);
        assert!(!idle.wanted(t0));
    }

    #[test]
    fn a_claim_goes_out_once_and_not_after_a_stop() {
        let t0 = Instant::now();
        let mut life = Lifecycle::default();
        life.on(Playback::Claim, t0);
        assert!(life.take_claim(t0));
        assert!(!life.take_claim(t0), "only once");

        life.on(Playback::Claim, t0);
        life.on(Playback::Stopped, t0);
        assert!(!life.take_claim(t0), "nothing plays here anymore");

        // Lost before it went out: sent on the next connection, while still playing.
        life.on(Playback::Claim, t0);
        assert!(life.take_claim(t0));
        life.keep_claim();
        assert!(life.take_claim(t0));
    }

    #[test]
    fn notifications_right_after_a_claim_are_ignored() {
        let t0 = Instant::now();
        let mut life = Lifecycle::default();
        assert!(!life.ignores_notification(t0));
        life.on(Playback::Claim, t0);
        life.take_claim(t0);
        assert!(life.ignores_notification(t0 + Duration::from_millis(500)));
        assert!(!life.ignores_notification(t0 + CLAIM_GRACE));
    }

    #[test]
    fn signed_out_waits_for_the_player() {
        let t0 = Instant::now();
        let mut life = Lifecycle::default();
        life.on(Playback::Claim, t0);
        life.give_up();
        assert!(!life.wanted(t0));
        assert!(!life.take_claim(t0));
        life.on(Playback::Playing, t0);
        assert!(life.wanted(t0));
    }
}
