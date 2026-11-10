//! Mode exclusif WASAPI (#245) : le moteur ouvre le périphérique pour lui seul. La
//! normalisation est suspendue (aucun filtre posé) ; le volume logiciel reste possible,
//! la chaîne audio signale alors que le signal n'est plus bit-perfect.

use super::actor::Player;
use crate::engine::Op;
use crate::i18n::{tr, trf};

/// Le moteur sait-il ouvrir une sortie en exclusif ? mpv sous Windows seulement : Media3
/// n'a pas de mode exclusif utilisable.
pub(super) fn supported() -> bool {
    cfg!(all(desktop, windows))
}

impl Player {
    /// Réglage changé : appliqué à chaud (mpv rouvre la sortie, un micro-blanc est normal),
    /// puis la normalisation est retirée ou rétablie sur le titre en cours.
    pub(super) async fn set_exclusive(&mut self, on: bool) {
        let on = on && supported();
        if on == self.state.exclusive {
            return;
        }
        tracing::info!("mode exclusif : {on}");
        self.state.exclusive = on;
        self.engine_op(Op::Exclusive(on)).await;
        let before = self.normalizer.effective();
        self.normalizer.bypass = on;
        self.on_normalization_changed(before).await;
    }

    /// La sortie ne s'ouvre pas en exclusif (périphérique déjà pris par une autre app, ou
    /// format refusé) : lecture arrêtée en pause sur le titre, avec un message clair. Pas de
    /// bascule silencieuse en partagé : Lecture réessaie à la même position.
    pub(super) async fn on_exclusive_failed(&mut self) {
        tracing::warn!("ouverture de la sortie en mode exclusif impossible");
        self.engine_op(Op::Stop).await;
        self.loaded = false;
        self.next_loaded = None;
        self.preloading = false;
        self.preload_req += 1;
        self.state.paused = true;
        let message = match self.output.describe().name {
            Some(name) => trf!(
                "Impossible d'ouvrir « {name} » en mode exclusif : une autre application l'utilise peut-être",
                "Cannot open “{name}” in exclusive mode: another application may be using it"
            ),
            None => tr(
                "Impossible d'ouvrir la sortie audio en mode exclusif : une autre application l'utilise peut-être",
                "Cannot open the audio output in exclusive mode: another application may be using it",
            )
            .into(),
        };
        self.toast("error", message);
    }
}
