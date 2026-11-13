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
mod tests {
    use super::*;
    use crate::tidal::models::{AlbumRef, ArtistRef, TrackId};
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn track(id: u64, title: &str, artist: &str) -> Track {
        Track {
            id: TrackId(id),
            title: title.into(),
            version: None,
            duration: 200,
            artists: vec![ArtistRef {
                id,
                name: artist.into(),
            }],
            album: AlbumRef {
                id,
                title: String::new(),
                cover_url: None,
                cover_small_url: None,
                cover_large_url: None,
                color: None,
            },
            explicit: false,
            ai: false,
            hires: false,
            isrc: None,
        }
    }

    fn pool(n: u64) -> Vec<Track> {
        (1..=n)
            .map(|i| track(i, &format!("Song {i}"), &format!("Artist {i}")))
            .collect()
    }

    /// Réponse de l'hôte seul : la manche révélée.
    fn solo(game: &mut Game, answer: Answer) -> Option<RoundResult> {
        game.answer(HOST_KEY, None, answer).ok()?;
        game.results().last().cloned()
    }

    fn rng() -> StdRng {
        StdRng::seed_from_u64(7)
    }

    #[test]
    fn needs_four_different_titles() {
        let mut rng = rng();
        assert_eq!(
            Game::new(pool(3), Mode::Typing, 5, Party::SOLO, &mut rng).err(),
            Some(GameError::TooFewTracks)
        );
        // Doublons et éditions du même titre ne comptent qu'une fois.
        let mut same = pool(3);
        same.push(track(1, "Song 1", "Artist 1"));
        same.push(track(9, "Song 1 (Remastered)", "Artist 1"));
        assert!(Game::new(same, Mode::Typing, 5, Party::SOLO, &mut rng).is_err());
        assert!(Game::new(pool(4), Mode::Typing, 5, Party::SOLO, &mut rng).is_ok());
    }

    #[test]
    fn rounds_are_drawn_without_replacement() {
        let mut rng = rng();
        let mut game = Game::new(pool(6), Mode::Typing, 10, Party::SOLO, &mut rng).unwrap();
        assert_eq!(game.rounds(), 6, "pas plus de manches que de titres");
        let mut seen = Vec::new();
        loop {
            seen.push(game.track().unwrap().id);
            solo(&mut game, Answer::Pass);
            if !game.next_round(&mut rng) {
                break;
            }
        }
        seen.sort_by_key(|id| id.0);
        seen.dedup();
        assert_eq!(seen.len(), 6);
        assert_eq!(game.phase, Phase::Finished);
        assert!(game.track().is_none());
        assert_eq!(game.round_number(), 6);
    }

    #[test]
    fn typing_scores_title_and_artist() {
        let mut rng = rng();
        let mut game = Game::new(pool(5), Mode::Typing, 3, Party::SOLO, &mut rng).unwrap();
        let t = game.track().unwrap().clone();
        assert!(game.hides_track());
        let r = solo(
            &mut game,
            Answer::Text {
                text: t.title.clone(),
            },
        )
        .unwrap();
        assert_eq!((r.title, r.artist, r.points), (true, false, 2));
        assert!(!game.hides_track());
        // Une seule réponse par manche.
        assert!(solo(&mut game, Answer::Pass).is_none());

        assert!(game.next_round(&mut rng));
        let t = game.track().unwrap().clone();
        let text = format!("{} {}", t.artists[0].name, t.title);
        assert_eq!(solo(&mut game, Answer::Text { text }).unwrap().points, 3);
        assert_eq!(game.host_score(), (5, 2, 2));

        assert!(game.next_round(&mut rng));
        solo(
            &mut game,
            Answer::Text {
                text: "rien à voir".into(),
            },
        );
        assert_eq!(game.host_score(), (5, 0, 2));
        assert!(!game.next_round(&mut rng));
        assert_eq!(game.results().len(), 3);
    }

    #[test]
    fn choices_hold_the_answer_once() {
        let mut rng = rng();
        let mut game = Game::new(pool(8), Mode::Choices, 4, Party::SOLO, &mut rng).unwrap();
        for _ in 0..4 {
            let id = game.track().unwrap().id;
            let choices = game.choices().to_vec();
            assert_eq!(choices.len(), 4);
            let right: Vec<usize> = choices
                .iter()
                .enumerate()
                .filter(|(_, c)| c.title == format!("Song {}", id.0))
                .map(|(i, _)| i)
                .collect();
            assert_eq!(right.len(), 1);
            let r = solo(&mut game, Answer::Choice { index: right[0] }).unwrap();
            assert_eq!(
                (r.points, r.chosen, r.correct),
                (3, Some(right[0]), Some(right[0]))
            );
            game.next_round(&mut rng);
        }
        assert_eq!(game.host_score().0, 12);
    }

    #[test]
    fn a_wrong_choice_scores_nothing() {
        let mut rng = rng();
        let mut game = Game::new(pool(4), Mode::Choices, 1, Party::SOLO, &mut rng).unwrap();
        let wrong = (0..4).find(|i| game.choices()[*i].title != game.track().unwrap().title);
        let r = solo(
            &mut game,
            Answer::Choice {
                index: wrong.unwrap(),
            },
        )
        .unwrap();
        assert_eq!(r.points, 0);
        // Une proposition n'est pas une réponse valable en saisie libre.
        let mut game = Game::new(pool(4), Mode::Typing, 1, Party::SOLO, &mut rng).unwrap();
        assert_eq!(
            solo(&mut game, Answer::Choice { index: 0 }).unwrap().points,
            0
        );
    }

    #[test]
    fn an_unplayable_track_is_replaced() {
        let mut rng = rng();
        let mut game = Game::new(pool(5), Mode::Typing, 5, Party::SOLO, &mut rng).unwrap();
        let first = game.track().unwrap().id;
        assert!(game.replace_track(&mut rng));
        assert_ne!(game.track().unwrap().id, first);
        assert_eq!(game.round_number(), 1);
        // Plus rien pour remplacer : la partie s'arrête sur les manches jouées.
        solo(&mut game, Answer::Pass);
        assert!(game.next_round(&mut rng));
        while game.replace_track(&mut rng) {}
        assert_eq!(game.phase, Phase::Finished);
        assert_eq!(game.rounds(), 1);
    }

    const JAM: Party = Party {
        jam: true,
        host_plays: true,
    };

    #[test]
    fn with_guests_the_round_waits_for_everyone() {
        let mut rng = rng();
        let mut game = Game::new(pool(6), Mode::Typing, 3, JAM, &mut rng).unwrap();
        let title = game.track().unwrap().title.clone();
        assert_eq!(game.answer("lea", Some("Léa"), Answer::Pass), Ok(1));
        assert!(game.hides_track(), "l'hôte n'a pas répondu");
        assert_eq!(
            game.answer(
                HOST_KEY,
                None,
                Answer::Text {
                    text: title.clone()
                }
            ),
            Ok(0)
        );
        // Tous les joueurs connus ont répondu : révélée.
        assert!(!game.hides_track());
        let r = game.results().last().unwrap();
        assert_eq!(r.points, 3, "titre + bonus du plus rapide");
        assert_eq!(r.players.len(), 2);
        assert_eq!(
            game.answer("max", None, Answer::Pass),
            Err(Refusal::Closed),
            "trop tard"
        );

        // L'hôte révèle sans attendre Léa.
        assert!(game.next_round(&mut rng));
        let title = game.track().unwrap().title.clone();
        game.answer(HOST_KEY, None, Answer::Text { text: title })
            .unwrap();
        assert!(game.hides_track());
        assert!(game.reveal());
        let lea = game
            .results()
            .last()
            .unwrap()
            .players
            .iter()
            .find(|p| p.id == 1)
            .cloned();
        assert_eq!(lea.map(|p| (p.answered, p.points)), Some((false, 0)));
        assert_eq!(game.host_score(), (6, 2, 2));
    }

    #[test]
    fn a_host_who_watches_reveals_on_the_guests() {
        let mut rng = rng();
        let party = Party {
            jam: true,
            host_plays: false,
        };
        let mut game = Game::new(pool(6), Mode::Typing, 2, party, &mut rng).unwrap();
        assert_eq!(
            game.answer(HOST_KEY, None, Answer::Pass),
            Err(Refusal::Closed)
        );
        game.answer("lea", None, Answer::Pass).unwrap();
        assert!(!game.hides_track(), "Léa était la seule joueuse");
        assert_eq!(game.results().last().unwrap().points, 0);
        // Seul, un invité ne peut pas répondre.
        let mut solo_game = Game::new(pool(6), Mode::Typing, 2, Party::SOLO, &mut rng).unwrap();
        assert_eq!(
            solo_game.answer("lea", None, Answer::Pass),
            Err(Refusal::Closed)
        );
    }

    #[test]
    fn clips_avoid_the_edges() {
        let mut rng = rng();
        for _ in 0..200 {
            let start = clip_start(240, &mut rng);
            assert!((48.0..=215.0).contains(&start), "{start}");
            let start = clip_start(600, &mut rng);
            assert!((60.0..=575.0).contains(&start), "{start}");
        }
        assert_eq!(clip_start(20, &mut rng), 2.0);
        assert_eq!(clip_start(10, &mut rng), 0.0);
    }
}
