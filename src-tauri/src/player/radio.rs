//! Radio automatique en fin de file (#78, réglage `PlaybackSettings::auto_radio`).
//!
//! Demandée au moment du préchargement, quand rien ne suit le titre courant : la radio
//! arrive avant la fin du titre et son premier titre est préchargé comme un autre
//! (gapless conservé). La logique de file est dans `Queue::extend_with_radio`.

use super::actor::{Cmd, Player};
use crate::tidal::models::{RadioSeed, TrackId};

#[derive(Debug, Default)]
pub(super) struct AutoRadio {
    pub(super) enabled: bool,
    /// Compteur contre les réponses périmées.
    req: u64,
    /// Titre dont la radio a déjà été demandée : une seule demande par titre, même en
    /// échec, tant que la file ne change pas (`Player::mark_queue_dirty` l'efface).
    pub(super) asked: Option<TrackId>,
    pending: bool,
    /// La lecture s'est arrêtée en fin de file avant la réponse : elle repart sur la radio.
    waiting: bool,
}

impl AutoRadio {
    pub(super) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ..Self::default()
        }
    }
}

impl Player {
    /// Rien ne suit le titre courant : demande sa radio, hors de l'acteur.
    pub(super) fn maybe_request_radio(&mut self) {
        let Some(seed) = self.queue.current() else {
            return;
        };
        if !self.radio.enabled || !self.queue.wants_radio() || self.radio.asked == Some(seed) {
            return;
        }
        self.radio.asked = Some(seed);
        self.radio.req += 1;
        self.radio.pending = true;
        let (tidal, tx, request) = (self.tidal.clone(), self.tx.clone(), self.radio.req);
        tauri::async_runtime::spawn(async move {
            let result = tidal
                .radio(RadioSeed::Track, seed.0)
                .await
                .map(|page| {
                    // Le filtre de contenu (#368) vaut aussi pour la radio automatique.
                    crate::content::filtered(page.tracks)
                        .into_iter()
                        .map(|t| t.id)
                        .collect()
                })
                .map_err(|e| e.to_string());
            let _ = tx
                .send(Cmd::RadioResolved {
                    request,
                    seed,
                    result,
                })
                .await;
        });
    }

    /// Fin de file atteinte (dernier titre terminé, rien de préchargé) : la lecture
    /// reprendra à l'arrivée de la radio si elle est en route.
    pub(super) fn on_queue_end(&mut self) {
        self.maybe_request_radio();
        self.radio.waiting = self.radio.pending;
        // Sans radio en route, la file est vraiment terminée (carton « Fin », #399).
        self.state.ended = !self.radio.waiting;
    }

    pub(super) async fn on_radio_resolved(
        &mut self,
        request: u64,
        seed: TrackId,
        result: Result<Vec<TrackId>, String>,
    ) {
        if request != self.radio.req {
            return;
        }
        self.radio.pending = false;
        let waiting = std::mem::take(&mut self.radio.waiting);
        let tracks = match result {
            Ok(tracks) => tracks,
            Err(e) => {
                // La file attendait cette radio : elle s'arrête là.
                self.state.ended |= waiting;
                return tracing::warn!("radio de {seed} indisponible : {e}");
            }
        };
        if !self.radio.enabled || !self.queue.extend_with_radio(seed, &tracks) {
            self.state.ended |= waiting;
            return;
        }
        tracing::info!("file prolongée par la radio de {seed}");
        self.mark_queue_dirty();
        if waiting {
            if let Some(id) = self.queue.advance() {
                self.load_current(id, 0.0);
            }
        } else {
            self.maybe_preload();
        }
    }
}
