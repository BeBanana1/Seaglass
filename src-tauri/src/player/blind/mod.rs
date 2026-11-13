//! Blind test (#434) : l'acteur joue des extraits d'une liste et masque le titre tant que la
//! manche n'est pas révélée. La file d'avant la partie est mise de côté (`saved`) : c'est
//! elle qui est sauvegardée, affichée et modifiée par « Ajouter à la file » pendant la
//! partie, et elle revient en pause à la fin.

mod answer;
mod game;

use std::borrow::Cow;

use serde::Serialize;
use tauri::Emitter;
use tokio::sync::oneshot;

use super::actor::{Cmd, Player};
use super::queue::Queue;
use super::state::PlayerState;
use crate::engine::Op;
use crate::i18n::trf;
use crate::tidal::models::{Track, TrackId};
pub use game::{Answer, Mode};
use game::{CLIP_LENGTH, Choice, Game, GameError, MIN_POOL, Phase, ROUND_POINTS, RoundResult};

/// Manches possibles par partie.
pub const MAX_ROUNDS: usize = 50;

pub enum BlindCmd {
    Start {
        tracks: Vec<TrackId>,
        mode: Mode,
        rounds: usize,
        /// Nom de la liste d'origine, affiché.
        title: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Answer(Answer),
    /// Réécoute l'extrait depuis son début.
    Replay,
    /// Manche suivante (ou passer, si la manche n'est pas révélée).
    Next,
    Quit,
    Get(oneshot::Sender<Option<BlindTestView>>),
}

pub(super) struct BlindTest {
    game: Game,
    title: String,
    /// File d'avant la partie et position de son titre courant.
    pub(super) saved: Queue,
    saved_position: f64,
}

/// État de la partie envoyé à l'UI (`blind-test`). Le titre de la manche en cours n'y est
/// qu'une fois révélé (`result`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlindTestView {
    title: String,
    mode: Mode,
    phase: Phase,
    round: usize,
    rounds: usize,
    score: u32,
    max_score: u32,
    streak: u32,
    best_streak: u32,
    clip_start: f64,
    clip_length: f64,
    choices: Vec<Choice>,
    /// Manches révélées, la dernière étant la manche en cours une fois révélée.
    results: Vec<RoundResult>,
}

impl BlindTest {
    fn view(&self) -> BlindTestView {
        let g = &self.game;
        BlindTestView {
            title: self.title.clone(),
            mode: g.mode,
            phase: g.phase,
            round: g.round_number(),
            rounds: g.rounds(),
            score: g.score,
            max_score: g.rounds() as u32 * ROUND_POINTS,
            streak: g.streak,
            best_streak: g.best_streak,
            clip_start: g.clip_start(),
            clip_length: CLIP_LENGTH,
            choices: g.choices().to_vec(),
            results: g.results().to_vec(),
        }
    }
}

/// État du lecteur tel qu'il sort de l'acteur : sans le titre tant que la manche n'est pas
/// révélée.
pub(super) fn public_state(state: &PlayerState, masked: bool) -> Cow<'_, PlayerState> {
    if !masked {
        return Cow::Borrowed(state);
    }
    Cow::Owned(PlayerState {
        track: None,
        hires_offer: None,
        soundtrack: false,
        ..state.clone()
    })
}

impl Player {
    pub(super) async fn blind_cmd(&mut self, cmd: BlindCmd) {
        match cmd {
            BlindCmd::Start {
                tracks,
                mode,
                rounds,
                title,
                reply,
            } => {
                let started = self.start_blind_test(tracks, mode, rounds, title);
                let _ = reply.send(started);
            }
            BlindCmd::Answer(answer) => self.blind_answer(answer).await,
            BlindCmd::Replay => self.blind_replay().await,
            BlindCmd::Next => self.blind_next().await,
            BlindCmd::Quit => self.end_blind_test().await,
            BlindCmd::Get(reply) => {
                let _ = reply.send(self.blind.as_ref().map(BlindTest::view));
            }
        }
    }

    /// Pendant une partie, Suivant / Précédent / Lecture pilotent la partie, et lancer autre
    /// chose (contexte, titre de la file, transfert) la termine d'abord. Renvoie la commande
    /// qui reste à traiter normalement.
    pub(super) async fn blind_route(&mut self, cmd: Cmd) -> Option<Cmd> {
        if self.blind.is_none() {
            return Some(cmd);
        }
        match cmd {
            Cmd::Next => self.blind_next().await,
            Cmd::Previous => self.blind_replay().await,
            Cmd::TogglePause | Cmd::SetPaused(false) if self.blind_wants_replay() => {
                self.blind_replay().await
            }
            Cmd::PlayTracks { .. }
            | Cmd::PlayFromQueue { .. }
            | Cmd::TakeHandoff(_)
            | Cmd::ApplyHandoff(..) => {
                self.end_blind_test().await;
                return Some(cmd);
            }
            cmd => return Some(cmd),
        }
        None
    }

    /// Le titre courant doit être masqué à la sortie de l'acteur.
    pub(super) fn masks_track(&self) -> bool {
        self.blind.as_ref().is_some_and(|b| b.game.hides_track())
    }

    /// File de l'utilisateur : celle mise de côté pendant une partie.
    pub(super) fn main_queue(&self) -> &Queue {
        self.blind.as_ref().map_or(&self.queue, |b| &b.saved)
    }

    pub(super) fn main_queue_mut(&mut self) -> &mut Queue {
        match &mut self.blind {
            Some(b) => &mut b.saved,
            None => &mut self.queue,
        }
    }

    /// Position à sauvegarder : celle d'avant la partie pendant une partie.
    pub(super) fn main_position(&self) -> f64 {
        self.blind
            .as_ref()
            .map_or(self.state.time_pos, |b| b.saved_position)
    }

    pub(super) fn emit_blind_test(&self) {
        let view = self.blind.as_ref().map(BlindTest::view);
        if let Err(e) = self.app.emit("blind-test", view) {
            tracing::warn!("émission blind-test : {e}");
        }
    }

    fn start_blind_test(
        &mut self,
        tracks: Vec<TrackId>,
        mode: Mode,
        rounds: usize,
        title: String,
    ) -> Result<(), String> {
        let pool: Vec<Track> = crate::content::filtered(self.tidal.cache.get_many(tracks));
        let game = Game::new(pool, mode, rounds.min(MAX_ROUNDS), &mut self.rng).map_err(
            |GameError::TooFewTracks| {
                trf!(
                    "il faut au moins {MIN_POOL} titres différents pour un blind test",
                    "a blind test needs at least {MIN_POOL} different tracks"
                )
            },
        )?;
        // Une nouvelle partie pendant une partie garde la file d'avant la première.
        let (saved, saved_position) = match self.blind.take() {
            Some(previous) => (previous.saved, previous.saved_position),
            None => (std::mem::take(&mut self.queue), self.state.time_pos),
        };
        tracing::info!("blind test : {} manches ({mode:?})", game.rounds());
        self.blind = Some(BlindTest {
            game,
            title,
            saved,
            saved_position,
        });
        self.play_round();
        Ok(())
    }

    /// Charge l'extrait de la manche en cours, seul dans la file du moteur.
    fn play_round(&mut self) {
        self.blind_dirty = true;
        let Some(blind) = &self.blind else {
            return;
        };
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

    /// Position reçue : l'extrait s'arrête de lui-même au bout de `CLIP_LENGTH`.
    pub(super) async fn blind_on_position(&mut self) {
        let Some(blind) = &self.blind else {
            return;
        };
        if blind.game.hides_track()
            && self.loaded
            && !self.state.paused
            && self.state.time_pos >= blind.game.clip_end()
        {
            // Fixé tout de suite : les positions suivantes ne redemandent pas la pause.
            self.state.paused = true;
            self.engine_op(Op::SetPaused(true)).await;
        }
    }

    /// Lecture demandée (bouton, touche média) sur un extrait terminé : il repart du début.
    pub(super) fn blind_wants_replay(&self) -> bool {
        self.blind.as_ref().is_some_and(|b| {
            b.game.hides_track() && self.state.paused && self.state.time_pos >= b.game.clip_end()
        })
    }

    async fn blind_answer(&mut self, answer: Answer) {
        let Some(blind) = &mut self.blind else {
            return;
        };
        if blind.game.answer(answer).is_none() {
            return;
        }
        self.blind_dirty = true;
        // Manche révélée : le titre continue jusqu'à la suivante.
        if self.loaded && self.state.paused {
            self.engine_op(Op::SetPaused(false)).await;
        }
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
            self.load_current(id, start);
        }
    }

    /// Suivant : passe la manche si elle n'est pas révélée, sinon lance la suivante (ou
    /// termine la partie).
    pub(super) async fn blind_next(&mut self) {
        let Some(phase) = self.blind.as_ref().map(|b| b.game.phase) else {
            return;
        };
        match phase {
            Phase::Guess => self.blind_answer(Answer::Pass).await,
            Phase::Revealed => {
                let more = self
                    .blind
                    .as_mut()
                    .is_some_and(|b| b.game.next_round(&mut self.rng));
                if more {
                    self.play_round();
                } else {
                    self.on_blind_finished().await;
                }
            }
            Phase::Finished => {}
        }
    }

    async fn on_blind_finished(&mut self) {
        self.blind_dirty = true;
        if self.loaded && !self.state.paused {
            self.engine_op(Op::SetPaused(true)).await;
        }
    }

    /// Titre de la manche illisible : un autre le remplace. Vrai si la partie a pris
    /// l'échec en charge.
    pub(super) async fn blind_track_failed(&mut self) -> bool {
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

    /// Fin de la partie : la file d'avant revient, en pause à sa position, sans être
    /// chargée (comme au lancement de l'app).
    pub(super) async fn end_blind_test(&mut self) {
        let Some(blind) = self.blind.take() else {
            return;
        };
        self.blind_dirty = true;
        if self.engine.is_ok() && (self.loaded || self.state.loading) {
            self.engine_op(Op::Stop).await;
        }
        // Les résolutions en route pour la partie sont ignorées à leur arrivée.
        self.load_req += 1;
        self.preload_req += 1;
        self.preloading = false;
        self.next_loaded = None;
        self.loaded = false;
        self.queue = blind.saved;
        let track = self.queue.current().and_then(|id| self.tidal.cache.get(id));
        let s = &mut self.state;
        s.duration = track.as_ref().map_or(0.0, |t| f64::from(t.duration));
        s.track = track;
        s.time_pos = blind.saved_position;
        s.paused = true;
        s.loading = false;
        s.ended = false;
        s.stream = None;
        s.hires_offer = None;
        s.normalization = None;
        self.mark_queue_dirty();
        self.persist_position = true;
        self.apply_loop_file().await;
        tracing::info!("blind test terminé, file restaurée");
    }
}
