//! Partie de blind test (#434) : tirage des manches, extraits, réponses et score. Logique
//! pure, sans moteur ni réseau ; l'acteur la pilote depuis `blind/mod.rs`.

use rand::Rng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use super::answer::{self, title_key};
use super::players::{HOST_KEY, Pending, PlayerResult, Players, Refusal};
use crate::tidal::models::Track;

/// Durée d'un extrait, en secondes.
pub const CLIP_LENGTH: f64 = 15.0;
/// Titres différents nécessaires : quatre propositions par manche.
pub const MIN_POOL: usize = 4;
const CHOICES: usize = 4;
/// Points d'un titre trouvé, d'un artiste trouvé ; une bonne proposition vaut les deux.
const TITLE_POINTS: u32 = 2;
const ARTIST_POINTS: u32 = 1;
pub const ROUND_POINTS: u32 = TITLE_POINTS + ARTIST_POINTS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// Saisie libre du titre et / ou de l'artiste.
    Typing,
    /// Quatre propositions « titre — artiste ».
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
    /// Passer : la manche est révélée sans point.
    Pass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    /// Extrait en cours, réponse attendue : le titre est masqué partout.
    Guess,
    Revealed,
    Finished,
}

/// Une proposition du mode `Choices`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub title: String,
    pub artist: String,
}

/// Une manche révélée : la réponse de l'hôte (s'il joue) et le résultat de chacun.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundResult {
    pub track: Track,
    pub title: bool,
    pub artist: bool,
    /// Points de l'hôte, bonus du plus rapide compris.
    pub points: u32,
    /// Saisie libre telle que tapée.
    pub answer: Option<String>,
    /// Proposition choisie et bonne proposition (mode `Choices`).
    pub chosen: Option<usize>,
    pub correct: Option<usize>,
    /// Tous les joueurs, du plus de points au moins (#441).
    pub players: Vec<PlayerResult>,
}

/// Qui joue : l'hôte seul, ou avec les invités de la Jam (#441).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Party {
    pub jam: bool,
    /// L'hôte répond aussi ; sinon il anime seulement (à plusieurs).
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
    /// Titres candidats, dans l'ordre du tirage ; `next` est le prochain à jouer.
    pool: Vec<Track>,
    next: usize,
    /// Manches prévues (réduites si des titres ne peuvent pas être lus).
    rounds: usize,
    round: Option<Round>,
    pub phase: Phase,
    results: Vec<RoundResult>,
    players: Players,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameError {
    /// Moins de `MIN_POOL` titres différents.
    TooFewTracks,
}

impl Game {
    /// Nouvelle partie de `rounds` manches tirées dans `pool` (doublons retirés), première
    /// manche prête.
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
        // Seul, l'hôte joue forcément.
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

    /// Prépare la manche suivante avec le prochain titre du pool ; faux s'il n'y en a plus.
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

    /// Bonne proposition et trois autres titres du pool, aux noms différents, mélangés.
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

    /// Titre de la manche en cours.
    pub fn track(&self) -> Option<&Track> {
        self.round.as_ref().map(|r| &r.track)
    }

    /// Début de l'extrait de la manche en cours.
    pub fn clip_start(&self) -> f64 {
        self.round.as_ref().map_or(0.0, |r| r.clip_start)
    }

    pub fn clip_end(&self) -> f64 {
        self.clip_start() + CLIP_LENGTH
    }

    pub fn choices(&self) -> &[Choice] {
        self.round.as_ref().map_or(&[], |r| &r.choices)
    }

    /// Numéro de la manche en cours, à partir de 1.
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

    /// Score, série et meilleure série de l'hôte.
    pub fn host_score(&self) -> (u32, u32, u32) {
        self.players.host_score()
    }

    /// Le titre doit rester masqué (UI, intégrations système, Jam).
    pub fn hides_track(&self) -> bool {
        self.phase == Phase::Guess
    }

    /// Réponse d'un joueur (`HOST_KEY` pour l'hôte, clé de la Jam pour un invité) à la
    /// manche en cours. Seul, la manche est révélée aussitôt ; à plusieurs, quand tous les
    /// joueurs ont répondu. Renvoie l'identifiant du joueur.
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

    /// Révèle la manche en cours (tous ont répondu, l'hôte le demande, délai écoulé) ;
    /// faux hors de `Guess`.
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

    /// Manche suivante après une révélation ; faux si la partie est finie.
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

    /// Le titre de la manche en cours ne peut pas être lu : un autre le remplace, sans
    /// compter de manche. Faux s'il n'en reste pas : la partie s'arrête là.
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

/// Début d'un extrait au hasard, loin du début (souvent une intro) et de la fin : entre
/// 20 % du titre (au plus 60 s) et 10 s avant la fin de l'extrait. Titre court : au
/// début.
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
