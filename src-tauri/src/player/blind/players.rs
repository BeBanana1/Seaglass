//! Players of a blind test (#434, #441): the host and, through the Jam, the guests. Answers
//! of the current round, points, fastest-player bonus, streaks. Pure logic.

use serde::Serialize;

/// The host's key; the guests' keys are their Jam keys (`jam::members::owner_key`), which
/// never leave this module.
pub const HOST_KEY: &str = "host";
pub const HOST_ID: u32 = 0;
/// Most players per game, the host included.
pub const MAX_PLAYERS: usize = 32;
const NAME_CHARS: usize = 24;
/// Bonus for the first to find the title, with several players only.
pub const FASTEST_BONUS: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// No round in progress (revealed, finished) or another round.
    Closed,
    /// This player has already answered the round.
    Duplicate,
    /// No room left in the game.
    Full,
}

/// Answer kept until the reveal.
#[derive(Debug, Clone, PartialEq)]
pub struct Pending {
    pub title: bool,
    pub artist: bool,
    /// Points without the bonus.
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

/// Player as the host and the guests see them: never their key or their answer.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerView {
    pub id: u32,
    /// First name given in the Jam; `None`: the UI shows "Guest <id>".
    pub name: Option<String>,
    pub host: bool,
    pub score: u32,
    pub streak: u32,
    pub best_streak: u32,
    /// Has answered the current round.
    pub answered: bool,
}

/// A player's result for a revealed round.
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
    /// First to find the title (bonus included in `points`).
    pub fastest: bool,
}

#[derive(Debug, Default)]
pub struct Players {
    /// In order of arrival, the host first if they play.
    list: Vec<Contestant>,
    next_id: u32,
    /// Order of the round's answers, for the fastest-player bonus.
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

    /// Player of `key`, added on their first answer; the latest first name wins.
    fn join(&mut self, key: &str, name: Option<&str>) -> Result<usize, Refusal> {
        let name = clean_name(name);
        if let Some(i) = self.list.iter().position(|c| c.key == key) {
            if name.is_some() {
                self.list[i].name = name;
            }
            return Ok(i);
        }
        if key == HOST_KEY || self.list.len() >= MAX_PLAYERS {
            // A host who does not play does not join mid-game.
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

    /// Keeps the answer of `key`; returns the player's id.
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

    /// All known players have answered (at least one player).
    pub fn all_answered(&self) -> bool {
        !self.list.is_empty() && self.list.iter().all(|c| c.pending.is_some())
    }

    /// Reveal: points (bonus to the first who found the title if `bonus`), scores and
    /// streaks; a player without an answer loses their streak. The answers are forgotten.
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

    /// The host's answer to the current round.
    pub fn host_pending(&self) -> Option<&Pending> {
        self.host().and_then(|c| c.pending.as_ref())
    }

    fn host(&self) -> Option<&Contestant> {
        self.list.iter().find(|c| c.id == HOST_ID)
    }

    /// The host's score, streak and best streak (0 if they do not play).
    pub fn host_score(&self) -> (u32, u32, u32) {
        self.host()
            .map_or((0, 0, 0), |c| (c.score, c.streak, c.best_streak))
    }

    /// Ranking: best score first, then order of arrival.
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
        assert!(!p.all_answered(), "answers forgotten after the reveal");
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
        // Max does not answer the next round: his streak stays at zero, Léa's also falls
        // back.
        p.record("max", Some("Max"), right(true, false)).unwrap();
        assert!(!p.all_answered());
        p.settle(true);
        let views = p.views();
        assert_eq!(views[0].id, 1, "Léa in the lead (3 points)");
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
        assert!(!p.all_answered(), "nobody has played yet");
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
