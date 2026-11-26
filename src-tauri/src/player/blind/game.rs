//! Blind test game (#434): drawing the rounds, clips, answers and score. Pure logic, with
//! no engine or network; the actor drives it from `blind/mod.rs`.

use rand::Rng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use super::answer::{self, title_key};
use super::players::{HOST_KEY, Pending, PlayerResult, Players, Refusal};
use crate::tidal::models::Track;

/// Length of a clip, in seconds.
pub const CLIP_LENGTH: f64 = 15.0;
/// Distinct tracks needed: four choices per round.
pub const MIN_POOL: usize = 4;
const CHOICES: usize = 4;
/// Points for a title found, for an artist found; a right choice is worth both.
const TITLE_POINTS: u32 = 2;
const ARTIST_POINTS: u32 = 1;
pub const ROUND_POINTS: u32 = TITLE_POINTS + ARTIST_POINTS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// Free typing of the title and / or the artist.
    Typing,
    /// Four "title — artist" choices.
    Choices,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Answer {
    Text {
        text: String,
    },
    Choice {
        index: usize,
    },
    /// Skip: the round is revealed with no points.
    Pass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    /// Clip playing, answer awaited: the track is hidden everywhere.
    Guess,
    Revealed,
    Finished,
}

/// One choice of the `Choices` mode.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub title: String,
    pub artist: String,
}

/// A revealed round: the host's answer (if they play) and everyone's result.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundResult {
    pub track: Track,
    pub title: bool,
    pub artist: bool,
    /// The host's points, fastest bonus included.
    pub points: u32,
    /// Free typing as typed.
    pub answer: Option<String>,
    /// Choice picked and right choice (`Choices` mode).
    pub chosen: Option<usize>,
    pub correct: Option<usize>,
    /// All players, from most points to fewest (#441).
    pub players: Vec<PlayerResult>,
}

/// Who plays: the host alone, or with the Jam guests (#441).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Party {
    pub jam: bool,
    /// The host answers too; otherwise they only run the game (with several players).
    pub host_plays: bool,
}

#[cfg(test)]
impl Party {
    pub const SOLO: Self = Self {
        jam: false,
        host_plays: true,
    };
}

#[derive(Debug)]
struct Round {
    track: Track,
    clip_start: f64,
    choices: Vec<Choice>,
    correct: usize,
}

#[derive(Debug)]
pub struct Game {
    pub mode: Mode,
    pub party: Party,
    /// Candidate tracks, in draw order; `next` is the next one to play.
    pool: Vec<Track>,
    next: usize,
    /// Planned rounds (fewer if some tracks cannot be played).
    rounds: usize,
    round: Option<Round>,
    pub phase: Phase,
    results: Vec<RoundResult>,
    players: Players,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameError {
    /// Fewer than `MIN_POOL` distinct tracks.
    TooFewTracks,
}

impl Game {
    /// New game of `rounds` rounds drawn from `pool` (duplicates removed), first round
    /// ready.
    pub fn new(
        pool: Vec<Track>,
        mode: Mode,
        rounds: usize,
        party: Party,
        rng: &mut impl Rng,
    ) -> Result<Self, GameError> {
        let mut seen = std::collections::HashSet::new();
        let mut pool: Vec<Track> = pool
            .into_iter()
            .filter(|t| seen.insert(t.id) && t.duration > 0)
            .collect();
        let titles: std::collections::HashSet<String> =
            pool.iter().map(|t| title_key(&t.title)).collect();
        if titles.len() < MIN_POOL {
            return Err(GameError::TooFewTracks);
        }
        pool.shuffle(rng);
        // Alone, the host necessarily plays.
        let party = Party {
            host_plays: party.host_plays || !party.jam,
            ..party
        };
        let mut game = Self {
            mode,
            party,
            rounds: rounds.clamp(1, pool.len()),
            pool,
            next: 0,
            round: None,
            phase: Phase::Guess,
            results: Vec::new(),
            players: Players::new(party.host_plays),
        };
        game.deal(rng);
        Ok(game)
    }

    /// Sets up the next round with the pool's next track; false if there is none left.
    fn deal(&mut self, rng: &mut impl Rng) -> bool {
        let Some(track) = self.pool.get(self.next).cloned() else {
            self.round = None;
            return false;
        };
        self.next += 1;
        let (choices, correct) = self.choices_for(&track, rng);
        self.round = Some(Round {
            clip_start: clip_start(track.duration, rng),
            track,
            choices,
            correct,
        });
        self.phase = Phase::Guess;
        true
    }

    /// The right choice and three other pool tracks with different names, shuffled.
    fn choices_for(&self, track: &Track, rng: &mut impl Rng) -> (Vec<Choice>, usize) {
        if self.mode != Mode::Choices {
            return (Vec::new(), 0);
        }
        let mut keys = vec![title_key(&track.title)];
        let mut others: Vec<&Track> = self.pool.iter().filter(|t| t.id != track.id).collect();
        others.shuffle(rng);
        let mut picked = vec![track];
        for other in others {
            if picked.len() == CHOICES {
                break;
            }
            let key = title_key(&other.title);
            if !keys.contains(&key) {
                keys.push(key);
                picked.push(other);
            }
        }
        picked.shuffle(rng);
        let correct = picked.iter().position(|t| t.id == track.id).unwrap_or(0);
        (picked.into_iter().map(choice).collect(), correct)
    }

    /// Track of the current round.
    pub fn track(&self) -> Option<&Track> {
        self.round.as_ref().map(|r| &r.track)
    }

    /// Start of the current round's clip.
    pub fn clip_start(&self) -> f64 {
        self.round.as_ref().map_or(0.0, |r| r.clip_start)
    }

    pub fn clip_end(&self) -> f64 {
        self.clip_start() + CLIP_LENGTH
    }

    pub fn choices(&self) -> &[Choice] {
        self.round.as_ref().map_or(&[], |r| &r.choices)
    }

    /// Number of the current round, from 1.
    pub fn round_number(&self) -> usize {
        (self.results.len() + usize::from(self.phase == Phase::Guess)).max(1)
    }

    pub fn rounds(&self) -> usize {
        self.rounds
    }

    pub fn results(&self) -> &[RoundResult] {
        &self.results
    }

    pub fn players(&self) -> &Players {
        &self.players
    }

    /// The host's score, streak and best streak.
    pub fn host_score(&self) -> (u32, u32, u32) {
        self.players.host_score()
    }

    /// The track must stay hidden (UI, system integrations, Jam).
    pub fn hides_track(&self) -> bool {
        self.phase == Phase::Guess
    }

    /// A player's answer (`HOST_KEY` for the host, the Jam key for a guest) to the current
    /// round. Alone, the round is revealed at once; with several players, when they have
    /// all answered. Returns the player's id.
    pub fn answer(
        &mut self,
        key: &str,
        name: Option<&str>,
        answer: Answer,
    ) -> Result<u32, Refusal> {
        if self.phase != Phase::Guess || (key != HOST_KEY && !self.party.jam) {
            return Err(Refusal::Closed);
        }
        let round = self.round.as_ref().ok_or(Refusal::Closed)?;
        let pending = self.judge(round, answer);
        let id = self.players.record(key, name, pending)?;
        if !self.party.jam || self.players.all_answered() {
            self.reveal();
        }
        Ok(id)
    }

    fn judge(&self, round: &Round, answer: Answer) -> Pending {
        let mut pending = Pending {
            title: false,
            artist: false,
            points: 0,
            answer: None,
            chosen: None,
        };
        match answer {
            Answer::Text { text } => {
                let verdict = answer::check(&text, &round.track);
                pending.title = verdict.title;
                pending.artist = verdict.artist;
                pending.answer = Some(text);
            }
            Answer::Choice { index } if self.mode == Mode::Choices => {
                let right = index == round.correct;
                pending.title = right;
                pending.artist = right;
                pending.chosen = Some(index);
            }
            Answer::Choice { .. } | Answer::Pass => {}
        }
        pending.points =
            u32::from(pending.title) * TITLE_POINTS + u32::from(pending.artist) * ARTIST_POINTS;
        pending
    }

    /// Reveals the current round (all answered, the host asks for it, time is up); false
    /// outside `Guess`.
    pub fn reveal(&mut self) -> bool {
        if self.phase != Phase::Guess {
            return false;
        }
        let Some(round) = &self.round else {
            return false;
        };
        let host = self.players.host_pending().cloned();
        let players = self.players.settle(self.party.jam);
        let host_points = players.iter().find(|p| p.host).map_or(0, |p| p.points);
        self.results.push(RoundResult {
            track: round.track.clone(),
            title: host.as_ref().is_some_and(|h| h.title),
            artist: host.as_ref().is_some_and(|h| h.artist),
            points: host_points,
            answer: host.as_ref().and_then(|h| h.answer.clone()),
            chosen: host.as_ref().and_then(|h| h.chosen),
            correct: (self.mode == Mode::Choices).then_some(round.correct),
            players,
        });
        self.phase = Phase::Revealed;
        true
    }

    /// Next round after a reveal; false if the game is over.
    pub fn next_round(&mut self, rng: &mut impl Rng) -> bool {
        if self.phase != Phase::Revealed {
            return self.phase == Phase::Guess;
        }
        if self.results.len() >= self.rounds || !self.deal(rng) {
            self.finish();
            return false;
        }
        true
    }

    /// The current round's track cannot be played: another one replaces it, without counting
    /// a round. False if none is left: the game stops there.
    pub fn replace_track(&mut self, rng: &mut impl Rng) -> bool {
        if self.phase != Phase::Guess {
            return false;
        }
        if self.deal(rng) {
            return true;
        }
        self.rounds = self.results.len();
        self.finish();
        false
    }

    fn finish(&mut self) {
        self.round = None;
        self.phase = Phase::Finished;
    }
}

fn choice(track: &Track) -> Choice {
    Choice {
        title: track.title.clone(),
        artist: track
            .artists
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", "),
    }
}

/// Random clip start, away from the beginning (often an intro) and the end: between 20 % of
/// the track (60 s at most) and 10 s before the end of the clip. Short track: at the start.
pub fn clip_start(duration: u32, rng: &mut impl Rng) -> f64 {
    let duration = f64::from(duration);
    let low = (duration * 0.2).min(60.0);
    let high = duration - CLIP_LENGTH - 10.0;
    if high <= low {
        return ((duration - CLIP_LENGTH) / 2.0).max(0.0).floor();
    }
    rng.random_range(low..high).floor()
}

#[cfg(test)]
mod tests;
