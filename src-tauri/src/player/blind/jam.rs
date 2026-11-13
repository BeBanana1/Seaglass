//! Blind test à plusieurs (#441) : délai de révélation. Les réponses des invités arrivent
//! par `BlindCmd::GuestAnswer` (API de la Jam) et passent par `Player::blind_answer`.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::Player;

/// Après la fin de l'extrait, temps laissé aux joueurs avant la révélation automatique.
pub const REVEAL_AFTER: Duration = Duration::from_secs(20);

impl Player {
    /// Fin de l'extrait, à plusieurs : la révélation automatique est programmée, une fois
    /// par manche (une réécoute ne la repousse pas).
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

    /// Tick d'une seconde de l'acteur : révèle la manche dont le délai est écoulé.
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
