//! The blind test's clip on the engine side (#434, #445): loading the round's track alone,
//! following it until it plays, stopping it at the end of the clip, replaying it and
//! replacing a track that cannot be played.

use super::super::actor::Player;
use super::super::queue::Queue;
use crate::engine::Op;

/// Loading of the clip (#445). `loading` drops as soon as the URL is resolved, before the
/// engine plays: the UI would otherwise follow a position that does not move yet, then
/// jump back at the first real position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Clip {
    /// Clip asked for, file not opened by the engine yet.
    Waiting,
    /// File opened (`Event::Loaded`): the first position past the clip's start will say
    /// that it plays. A position received before `Loaded` may come from the previous
    /// round's track and does not count.
    Opened,
    Playing,
}

impl Player {
    /// Loads the current round's clip, alone in the engine's queue.
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

    /// File opened by the engine: the round's clip is about to start.
    pub(in crate::player) fn blind_on_loaded(&mut self) {
        if let Some(blind) = &mut self.blind
            && blind.clip == Clip::Waiting
        {
            blind.clip = Clip::Opened;
        }
    }

    /// Position received: the clip counts as playing from the first position past its start,
    /// and stops by itself after `CLIP_LENGTH`.
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
            // Set at once: the next positions do not ask for the pause again.
            self.state.paused = true;
            self.engine_op(Op::SetPaused(true)).await;
            self.start_reveal_countdown();
        }
    }

    /// Play asked (button, media key) on a finished clip: it starts again from the beginning.
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

    /// The round's track cannot be played: another one replaces it. True if the game has
    /// handled the failure.
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
