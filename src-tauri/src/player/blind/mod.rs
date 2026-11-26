//! Blind test (#434) : l'acteur joue des extraits d'une liste et masque le titre tant que la
//! manche n'est pas révélée. La file d'avant la partie est mise de côté (`saved`) : c'est
//! elle qui est sauvegardée, affichée et modifiée par « Ajouter à la file » pendant la
//! partie, et elle revient en pause à la fin. À plusieurs (#441), les invités de la Jam
//! répondent depuis leur téléphone (`jam.rs`).

mod answer;
mod clip;
mod game;
mod jam;
mod players;

use std::borrow::Cow;
use std::time::Instant;

use serde::Serialize;
use tauri::Emitter;
use tokio::sync::oneshot;

use super::actor::{Cmd, Player};
use super::queue::Queue;
use super::state::PlayerState;
use crate::engine::Op;
use crate::i18n::trf;
use crate::tidal::models::{Track, TrackId};
use clip::Clip;
pub use game::{Answer, Choice, Mode, Party, Phase, RoundResult};
use game::{CLIP_LENGTH, Game, GameError, MIN_POOL, ROUND_POINTS};
pub use players::{HOST_KEY, PlayerResult, PlayerView, Refusal};

/// Manches possibles par partie.
pub const MAX_ROUNDS: usize = 50;

pub enum BlindCmd {
    Start {
        tracks: Vec<TrackId>,
        mode: Mode,
        rounds: usize,
        /// Nom de la liste d'origine, affiché.
        title: String,
        /// Seul, ou avec les invités de la Jam (#441).
        party: Party,
        reply: oneshot::Sender<Result<(), String>>,
    },
    /// Réponse de l'hôte.
    Answer(Answer),
    /// Réponse d'un invité de la Jam (#441) à la manche `round` ; renvoie son identifiant
    /// de joueur.
    GuestAnswer {
        /// Clé de l'invité dans la Jam (jeton du navigateur ou adresse), jamais affichée.
        key: String,
        name: Option<String>,
        round: usize,
        answer: Answer,
        reply: oneshot::Sender<Result<u32, Refusal>>,
    },
    /// Réécoute l'extrait depuis son début.
    Replay,
    /// Révèle la manche sans attendre les autres réponses (à plusieurs).
    Reveal,
    /// Manche suivante (ou passer / révéler, si la manche n'est pas révélée).
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
    /// À plusieurs : révélation automatique, `REVEAL_AFTER` après la fin de l'extrait.
    reveal_at: Option<(Instant, i64)>,
    /// Où en est le chargement de l'extrait (#445).
    clip: Clip,
}

/// État de la partie envoyé à l'UI (`blind-test`). Le titre de la manche en cours n'y est
/// qu'une fois révélé (`result`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlindTestView {
    pub title: String,
    pub mode: Mode,
    pub phase: Phase,
    pub round: usize,
    pub rounds: usize,
    pub score: u32,
    pub max_score: u32,
    pub streak: u32,
    pub best_streak: u32,
    pub clip_start: f64,
    pub clip_length: f64,
    pub choices: Vec<Choice>,
    /// Manches révélées, la dernière étant la manche en cours une fois révélée.
    pub results: Vec<RoundResult>,
    pub party: Party,
    /// Classement (#441) ; l'hôte seul en solo.
    pub players: Vec<PlayerView>,
    /// Révélation automatique prévue (ms depuis l'epoch), à plusieurs.
    pub reveal_at: Option<i64>,
    /// L'extrait joue vraiment (#445) : l'anneau ne part qu'à ce moment.
    pub clip_playing: bool,
}

impl BlindTest {
    fn view(&self) -> BlindTestView {
        let g = &self.game;
        let (score, streak, best_streak) = g.host_score();
        BlindTestView {
            title: self.title.clone(),
            mode: g.mode,
            phase: g.phase,
            round: g.round_number(),
            rounds: g.rounds(),
            score,
            max_score: g.rounds() as u32 * ROUND_POINTS,
            streak,
            best_streak,
            clip_start: g.clip_start(),
            clip_length: CLIP_LENGTH,
            choices: g.choices().to_vec(),
            results: g.results().to_vec(),
            party: g.party,
            players: g.players().views(),
            reveal_at: self.reveal_at.map(|(_, ms)| ms),
            clip_playing: self.clip == Clip::Playing,
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
                party,
                reply,
            } => {
                let started = self.start_blind_test(tracks, mode, rounds, title, party);
                let _ = reply.send(started);
            }
            BlindCmd::Answer(answer) => {
                let _ = self.blind_answer(HOST_KEY, None, None, answer).await;
            }
            BlindCmd::GuestAnswer {
                key,
                name,
                round,
                answer,
                reply,
            } => {
                let result = self
                    .blind_answer(&key, name.as_deref(), Some(round), answer)
                    .await;
                let _ = reply.send(result);
            }
            BlindCmd::Reveal => self.blind_reveal().await,
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
        party: Party,
    ) -> Result<(), String> {
        let pool: Vec<Track> = crate::content::filtered(self.tidal.cache.get_many(tracks));
        let game = Game::new(pool, mode, rounds.min(MAX_ROUNDS), party, &mut self.rng).map_err(
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
        tracing::info!(
            "blind test : {} manches ({mode:?}, Jam : {})",
            game.rounds(),
            party.jam
        );
        self.blind = Some(BlindTest {
            game,
            title,
            saved,
            saved_position,
            reveal_at: None,
            clip: Clip::Waiting,
        });
        self.play_round();
        Ok(())
    }

    /// Réponse d'un joueur ; `round` (invités) écarte une réponse arrivée après la manche.
    async fn blind_answer(
        &mut self,
        key: &str,
        name: Option<&str>,
        round: Option<usize>,
        answer: Answer,
    ) -> Result<u32, Refusal> {
        let blind = self.blind.as_mut().ok_or(Refusal::Closed)?;
        if round.is_some_and(|r| r != blind.game.round_number()) {
            return Err(Refusal::Closed);
        }
        let id = blind.game.answer(key, name, answer)?;
        self.blind_dirty = true;
        if !self.masks_track() {
            self.on_blind_revealed().await;
        }
        Ok(id)
    }

    /// Manche révélée : le titre continue jusqu'à la suivante.
    async fn on_blind_revealed(&mut self) {
        if let Some(blind) = &mut self.blind {
            blind.reveal_at = None;
        }
        self.blind_dirty = true;
        if self.loaded && self.state.paused {
            self.engine_op(Op::SetPaused(false)).await;
        }
    }

    async fn blind_reveal(&mut self) {
        if self.blind.as_mut().is_some_and(|b| b.game.reveal()) {
            self.on_blind_revealed().await;
        }
    }

    /// Suivant : passe la manche (seul) ou la révèle (à plusieurs) si elle ne l'est pas,
    /// sinon lance la suivante (ou termine la partie).
    pub(super) async fn blind_next(&mut self) {
        let Some((phase, jam)) = self
            .blind
            .as_ref()
            .map(|b| (b.game.phase, b.game.party.jam))
        else {
            return;
        };
        match phase {
            Phase::Guess if jam => self.blind_reveal().await,
            Phase::Guess => {
                let _ = self.blind_answer(HOST_KEY, None, None, Answer::Pass).await;
            }
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
