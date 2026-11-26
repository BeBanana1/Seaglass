//! Multiplayer blind test (#441): reveal delay. The guests' answers arrive through
//! `BlindCmd::GuestAnswer` (the Jam's API) and go through `Player::blind_answer`.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::Player;

/// After the clip ends, time left to the players before the automatic reveal.
pub const REVEAL_AFTER: Duration = Duration::from_secs(20);

impl Player {
    /// End of the clip, with several players: the automatic reveal is scheduled, once per
    /// round (listening again does not push it back).
    pub(super) fn start_reveal_countdown(&mut self) {
        let Some(blind) = &mut self.blind else {
            return;
        };
        if !blind.game.party.jam || blind.reveal_at.is_some() {
            return;
        }
        let ms = SystemTime::now()
            .checked_add(REVEAL_AFTER)
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_millis() as i64);
        blind.reveal_at = Some((Instant::now() + REVEAL_AFTER, ms));
        self.blind_dirty = true;
    }

    /// The actor's one-second tick: reveals the round whose delay has elapsed.
    pub(in crate::player) async fn check_blind_deadline(&mut self) {
        let due = self
            .blind
            .as_ref()
            .and_then(|b| b.reveal_at)
            .is_some_and(|(at, _)| Instant::now() >= at);
        if due {
            self.blind_reveal().await;
        }
    }
}
