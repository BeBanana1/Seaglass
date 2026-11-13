//! Joueurs d'un blind test (#434, #441) : l'hôte et, par la Jam, les invités. Réponses de
//! la manche en cours, points, bonus du plus rapide, séries. Logique pure.

use serde::Serialize;

/// Clé de l'hôte ; celles des invités sont leurs clés de la Jam (`jam::members::owner_key`),
/// qui ne sortent jamais d'ici.
pub const HOST_KEY: &str = "host";
pub const HOST_ID: u32 = 0;
/// Joueurs au plus par partie, l'hôte compris.
pub const MAX_PLAYERS: usize = 32;
const NAME_CHARS: usize = 24;
/// Bonus du premier à trouver le titre, à plusieurs seulement.
pub const FASTEST_BONUS: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Pas de manche en cours (révélée, finie) ou autre manche.
    Closed,
    /// Ce joueur a déjà répondu à la manche.
    Duplicate,
    /// Plus de place dans la partie.
    Full,
}

/// Réponse retenue jusqu'à la révélation.
#[derive(Debug, Clone, PartialEq)]
pub struct Pending {
    pub title: bool,
    pub artist: bool,
    /// Points sans le bonus.
    pub points: u32,
    pub answer: Option<String>,
    pub chosen: Option<usize>,
}

#[derive(Debug, Clone)]
struct Contestant {
    id: u32,
    key: String,
    name: Option<String>,
    score: u32,
    streak: u32,
    best_streak: u32,
    pending: Option<Pending>,
}

/// Joueur tel que l'hôte et les invités le voient : jamais sa clé ni sa réponse.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerView {
    pub id: u32,
    /// Prénom donné dans la Jam ; `None` : l'UI affiche « Invité <id> ».
    pub name: Option<String>,
    pub host: bool,
    pub score: u32,
    pub streak: u32,
    pub best_streak: u32,
    /// A répondu à la manche en cours.
    pub answered: bool,
}

/// Résultat d'un joueur pour une manche révélée.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerResult {
    pub id: u32,
    pub name: Option<String>,
    pub host: bool,
    pub answered: bool,
    pub title: bool,
    pub artist: bool,
    pub points: u32,
    /// Premier à trouver le titre (bonus compris dans `points`).
    pub fastest: bool,
}

#[derive(Debug, Default)]
pub struct Players {
    /// Dans l'ordre d'arrivée, l'hôte d'abord s'il joue.
    list: Vec<Contestant>,
    next_id: u32,
    /// Ordre des réponses de la manche, pour le bonus du plus rapide.
    order: Vec<u32>,
}

fn clean_name(name: Option<&str>) -> Option<String> {
    let name: String = name?.chars().filter(|c| !c.is_control()).collect();
    let name: String = name.trim().chars().take(NAME_CHARS).collect();
    let name = name.trim_end();
    (!name.is_empty()).then(|| name.to_owned())
}

impl Players {
    pub fn new(host_plays: bool) -> Self {
        let mut players = Self {
            next_id: HOST_ID,
            ..Self::default()
        };
        if host_plays {
            players.list.push(Contestant {
                id: HOST_ID,
                key: HOST_KEY.into(),
                name: None,
                score: 0,
                streak: 0,
                best_streak: 0,
                pending: None,
            });
        }
        players
    }

    /// Joueur de `key`, ajouté à sa première réponse ; le dernier prénom l'emporte.
    fn join(&mut self, key: &str, name: Option<&str>) -> Result<usize, Refusal> {
        let name = clean_name(name);
        if let Some(i) = self.list.iter().position(|c| c.key == key) {
            if name.is_some() {
                self.list[i].name = name;
            }
            return Ok(i);
        }
        if key == HOST_KEY || self.list.len() >= MAX_PLAYERS {
            // L'hôte qui ne joue pas ne rejoint pas en cours de partie.
            return Err(if key == HOST_KEY {
                Refusal::Closed
            } else {
                Refusal::Full
            });
        }
        self.next_id += 1;
        self.list.push(Contestant {
            id: self.next_id,
            key: key.to_owned(),
            name,
            score: 0,
            streak: 0,
            best_streak: 0,
            pending: None,
        });
        Ok(self.list.len() - 1)
    }

    /// Retient la réponse de `key` ; renvoie l'identifiant du joueur.
    pub fn record(
        &mut self,
        key: &str,
        name: Option<&str>,
        pending: Pending,
    ) -> Result<u32, Refusal> {
        let i = self.join(key, name)?;
        let player = &mut self.list[i];
        if player.pending.is_some() {
            return Err(Refusal::Duplicate);
        }
        player.pending = Some(pending);
        self.order.push(player.id);
        Ok(player.id)
    }

    /// Tous les joueurs connus ont répondu (au moins un joueur).
    pub fn all_answered(&self) -> bool {
        !self.list.is_empty() && self.list.iter().all(|c| c.pending.is_some())
    }

    /// Révélation : points (bonus au premier qui a trouvé le titre si `bonus`), scores et
    /// séries ; un joueur sans réponse perd sa série. Les réponses sont oubliées.
    pub fn settle(&mut self, bonus: bool) -> Vec<PlayerResult> {
        let fastest = bonus
            .then(|| {
                self.order.iter().copied().find(|id| {
                    self.list
                        .iter()
                        .any(|c| c.id == *id && c.pending.as_ref().is_some_and(|p| p.title))
                })
            })
            .flatten();
        self.order.clear();
        let mut results = Vec::with_capacity(self.list.len());
        for c in &mut self.list {
            let pending = c.pending.take();
            let is_fastest = fastest == Some(c.id);
            let points = pending.as_ref().map_or(0, |p| p.points)
                + if is_fastest { FASTEST_BONUS } else { 0 };
            c.score += points;
            if points > 0 {
                c.streak += 1;
                c.best_streak = c.best_streak.max(c.streak);
            } else {
                c.streak = 0;
            }
            results.push(PlayerResult {
                id: c.id,
                name: c.name.clone(),
                host: c.id == HOST_ID,
                answered: pending.is_some(),
                title: pending.as_ref().is_some_and(|p| p.title),
                artist: pending.as_ref().is_some_and(|p| p.artist),
                points,
                fastest: is_fastest,
            });
        }
        results.sort_by(|a, b| b.points.cmp(&a.points).then(a.id.cmp(&b.id)));
        results
    }

    /// Réponse de l'hôte à la manche en cours.
    pub fn host_pending(&self) -> Option<&Pending> {
        self.host().and_then(|c| c.pending.as_ref())
    }

    fn host(&self) -> Option<&Contestant> {
        self.list.iter().find(|c| c.id == HOST_ID)
    }

    /// Score, série et meilleure série de l'hôte (0 s'il ne joue pas).
    pub fn host_score(&self) -> (u32, u32, u32) {
        self.host()
            .map_or((0, 0, 0), |c| (c.score, c.streak, c.best_streak))
    }

    /// Classement : meilleur score d'abord, puis ordre d'arrivée.
    pub fn views(&self) -> Vec<PlayerView> {
        let mut views: Vec<PlayerView> = self
            .list
            .iter()
            .map(|c| PlayerView {
                id: c.id,
                name: c.name.clone(),
                host: c.id == HOST_ID,
                score: c.score,
                streak: c.streak,
                best_streak: c.best_streak,
                answered: c.pending.is_some(),
            })
            .collect();
        views.sort_by(|a, b| b.score.cmp(&a.score).then(a.id.cmp(&b.id)));
        views
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn right(title: bool, artist: bool) -> Pending {
        Pending {
            title,
            artist,
            points: u32::from(title) * 2 + u32::from(artist),
            answer: None,
            chosen: None,
        }
    }

    #[test]
    fn the_fastest_right_title_gets_the_bonus() {
        let mut p = Players::new(true);
        assert_eq!(p.record("lea", Some("Léa"), right(false, true)), Ok(1));
        assert_eq!(p.record("max", Some("Max"), right(true, false)), Ok(2));
        assert_eq!(p.record(HOST_KEY, None, right(true, true)), Ok(HOST_ID));
        assert!(p.all_answered());
        let results = p.settle(true);
        let points: Vec<(u32, u32, bool)> = results
            .iter()
            .map(|r| (r.id, r.points, r.fastest))
            .collect();
        assert_eq!(
            points,
            vec![(HOST_ID, 3, false), (2, 3, true), (1, 1, false)]
        );
        assert!(!p.all_answered(), "réponses oubliées après la révélation");
    }

    #[test]
    fn no_bonus_alone() {
        let mut p = Players::new(true);
        p.record(HOST_KEY, None, right(true, false)).unwrap();
        assert_eq!(p.settle(false)[0].points, 2);
    }

    #[test]
    fn one_answer_per_round_and_streaks() {
        let mut p = Players::new(false);
        p.record("lea", None, right(true, false)).unwrap();
        assert_eq!(
            p.record("lea", None, right(true, true)),
            Err(Refusal::Duplicate)
        );
        p.record("max", None, right(false, false)).unwrap();
        p.settle(true);
        // Max ne répond pas à la manche suivante : sa série reste à zéro, celle de Léa
        // aussi retombe.
        p.record("max", Some("Max"), right(true, false)).unwrap();
        assert!(!p.all_answered());
        p.settle(true);
        let views = p.views();
        assert_eq!(views[0].id, 1, "Léa en tête (3 points)");
        assert_eq!(
            (views[0].score, views[0].streak, views[0].best_streak),
            (3, 0, 1)
        );
        assert_eq!((views[1].score, views[1].streak), (3, 1));
        assert_eq!(views[1].name.as_deref(), Some("Max"));
    }

    #[test]
    fn a_host_who_watches_cannot_answer() {
        let mut p = Players::new(false);
        assert_eq!(
            p.record(HOST_KEY, None, right(true, true)),
            Err(Refusal::Closed)
        );
        assert_eq!(p.host_score(), (0, 0, 0));
        assert!(!p.all_answered(), "personne n'a encore joué");
    }

    #[test]
    fn players_are_capped_and_names_cleaned() {
        let mut p = Players::new(true);
        for i in 1..MAX_PLAYERS {
            p.record(&format!("k{i}"), None, right(false, false))
                .unwrap();
        }
        assert_eq!(
            p.record("late", None, right(false, false)),
            Err(Refusal::Full)
        );
        let mut p = Players::new(false);
        p.record(
            "x",
            Some("  \u{7}Léa la très très longue invitée  "),
            right(false, false),
        )
        .unwrap();
        assert_eq!(
            p.views()[0].name.as_deref(),
            Some("Léa la très très longue")
        );
        let mut p = Players::new(false);
        p.record("y", Some("   "), right(false, false)).unwrap();
        assert_eq!(p.views()[0].name, None);
    }
}
