//! Transfert de la lecture entre deux appareils (#414) : la file, la position et les
//! métadonnées de ce qui s'affiche. Jamais d'URL de flux (signées, elles expirent) :
//! l'appareil qui reprend redemande les siennes.

use serde::{Deserialize, Serialize};

use super::actor::Player;
use super::queue::Queue;
use crate::tidal::models::Track;

/// Titres de la suite du contexte dont les métadonnées voyagent (l'UI en montre 50).
const UPCOMING_TRACKS: usize = 100;
/// Métadonnées acceptées au plus : au-delà, le reste est ignoré.
pub const MAX_TRACKS: usize = 2_500;

/// Ce qui passe d'un appareil à l'autre.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Handoff {
    pub queue: Queue,
    /// Position dans le titre courant, en secondes.
    pub position: f64,
    /// Métadonnées du titre courant, de la file manuelle et du début de la suite, pour
    /// un affichage immédiat ; les autres sont lues au moment de les jouer.
    pub tracks: Vec<Track>,
}

impl Handoff {
    /// Le transfert s'il est cohérent (file vérifiée, position bornée), sinon `None`.
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
    /// Instantané de la lecture, puis pause : TIDAL limite la lecture simultanée par
    /// compte, l'appareil qui reprend ne doit pas jouer en même temps. `None` si rien
    /// n'est en cours.
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
        tracing::info!(
            "lecture transférée : titre {} à {:.0} s",
            current.0,
            handoff.position
        );
        Some(handoff)
    }

    /// Reprend la lecture transférée : la file remplace celle d'ici, puis le titre courant
    /// est lu à la position reçue. `handoff` a déjà été vérifié (`Handoff::received`).
    pub(super) async fn apply_handoff(&mut self, handoff: Handoff) -> bool {
        let Some(id) = handoff.queue.current() else {
            return false;
        };
        self.tidal.cache.insert_all(&handoff.tracks);
        self.queue = handoff.queue;
        self.on_queue_changed().await;
        tracing::info!(
            "lecture reprise d'un autre appareil : titre {} à {:.0} s",
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
