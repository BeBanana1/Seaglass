//! The blind test's clip on the engine side (#434, #445): loading the round's track alone,
//! following it until it plays, stopping it at the end of the clip, replaying it and
//! replacing a track that cannot be played.

use super::super::actor::Player;
use super::super::queue::Queue;
use crate::engine::Op;

/// Chargement de l'extrait (#445). `loading` retombe dès que l'URL est résolue, avant que
/// le moteur joue : l'UI suivrait sinon une position qui n'avance pas encore, puis
/// sauterait en arrière à la première vraie position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Clip {
    /// Extrait demandé, fichier pas encore ouvert par le moteur.
    Waiting,
    /// Fichier ouvert (`Event::Loaded`) : la première position au-delà du début de
    /// l'extrait dira qu'il joue. Une position reçue avant `Loaded` peut venir du titre de
    /// la manche précédente et ne compte pas.
    Opened,
    Playing,
}

impl Player {
    /// Charge l'extrait de la manche en cours, seul dans la file du moteur.
    pub(super) fn play_round(&mut self) {
        self.blind_dirty = true;
        let Some(blind) = &mut self.blind else {
            return;
        };
        blind.reveal_at = None;
        blind.clip = Clip::Waiting;
        let (Some(track), start) = (blind.game.track(), blind.game.clip_start()) else {
            return;
        };
        let id = track.id;
        let mut queue = Queue::default();
        queue.set_context(vec![id], 0, None, &mut self.rng);
        self.queue = queue;
        self.mark_queue_dirty();
        self.load_current(id, start);
    }

    /// Fichier ouvert par le moteur : l'extrait de la manche va commencer.
    pub(in crate::player) fn blind_on_loaded(&mut self) {
        if let Some(blind) = &mut self.blind
            && blind.clip == Clip::Waiting
        {
            blind.clip = Clip::Opened;
        }
    }

    /// Position reçue : l'extrait est lancé à la première position qui dépasse son début,
    /// et s'arrête de lui-même au bout de `CLIP_LENGTH`.
    pub(in crate::player) async fn blind_on_position(&mut self) {
        let Some(blind) = &mut self.blind else {
            return;
        };
        if blind.clip == Clip::Opened
            && self.loaded
            && self.state.time_pos > blind.game.clip_start()
        {
            blind.clip = Clip::Playing;
            self.blind_dirty = true;
        }
        if blind.game.hides_track()
            && self.loaded
            && !self.state.paused
            && self.state.time_pos >= blind.game.clip_end()
        {
            // Fixé tout de suite : les positions suivantes ne redemandent pas la pause.
            self.state.paused = true;
            self.engine_op(Op::SetPaused(true)).await;
            self.start_reveal_countdown();
        }
    }

    /// Lecture demandée (bouton, touche média) sur un extrait terminé : il repart du début.
    pub(super) fn blind_wants_replay(&self) -> bool {
        self.blind.as_ref().is_some_and(|b| {
            b.game.hides_track() && self.state.paused && self.state.time_pos >= b.game.clip_end()
        })
    }

    pub(super) async fn blind_replay(&mut self) {
        let Some(blind) = &self.blind else {
            return;
        };
        let (start, id) = (blind.game.clip_start(), blind.game.track().map(|t| t.id));
        if self.loaded {
            self.state.time_pos = start;
            self.engine_op(Op::Seek(start)).await;
            self.engine_op(Op::SetPaused(false)).await;
        } else if let Some(id) = id
            && !self.state.loading
        {
            if let Some(blind) = &mut self.blind {
                blind.clip = Clip::Waiting;
                self.blind_dirty = true;
            }
            self.load_current(id, start);
        }
    }

    /// Titre de la manche illisible : un autre le remplace. Vrai si la partie a pris
    /// l'échec en charge.
    pub(in crate::player) async fn blind_track_failed(&mut self) -> bool {
        let Some(blind) = &mut self.blind else {
            return false;
        };
        self.loaded = false;
        if blind.game.replace_track(&mut self.rng) {
            self.play_round();
        } else {
            self.on_blind_finished().await;
        }
        true
    }
}
