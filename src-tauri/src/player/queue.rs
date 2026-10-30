//! File d'attente : logique pure, sans mpv ni réseau.
//!
//! - `context` : titres de l'album / playlist / recherche d'origine ;
//!   `order` : ordre de lecture (indices dans `context`) ; `pos` : curseur dans `order`.
//! - `manual` : file manuelle, prioritaire sur le contexte.
//! - `history` : titres précédents, indépendant du shuffle.
//! - `Repeat::One` est géré par mpv (`loop-file=inf`) : la file l'ignore.

use std::collections::VecDeque;

use rand::Rng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::tidal::models::TrackId;

const HISTORY_MAX: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    id: TrackId,
    /// Vrai si le titre vient du contexte (et non de la file manuelle).
    from_context: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Queue {
    context: Vec<TrackId>,
    order: Vec<usize>,
    pos: usize,
    manual: VecDeque<TrackId>,
    history: Vec<Entry>,
    current: Option<Entry>,
    repeat: Repeat,
    shuffle: bool,
}

impl Queue {
    pub fn current(&self) -> Option<TrackId> {
        self.current.map(|e| e.id)
    }

    pub fn repeat(&self) -> Repeat {
        self.repeat
    }

    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    pub fn has_previous(&self) -> bool {
        !self.history.is_empty()
    }

    /// Remplace le contexte et démarre sur `start`. La file manuelle est conservée.
    pub fn set_context(
        &mut self,
        tracks: Vec<TrackId>,
        start: usize,
        rng: &mut impl Rng,
    ) -> Option<TrackId> {
        let id = *tracks.get(start)?;
        self.push_history();
        self.order = (0..tracks.len()).collect();
        self.context = tracks;
        if self.shuffle {
            self.shuffle_around(start, rng);
        } else {
            self.pos = start;
        }
        self.current = Some(Entry {
            id,
            from_context: true,
        });
        Some(id)
    }

    /// Active : Fisher-Yates sur les autres titres, piste courante en tête.
    /// Désactive : ordre d'origine, curseur replacé sur la piste courante.
    pub fn set_shuffle(&mut self, on: bool, rng: &mut impl Rng) {
        if on == self.shuffle {
            return;
        }
        self.shuffle = on;
        let Some(&head) = self.order.get(self.pos) else {
            return;
        };
        if on {
            self.shuffle_around(head, rng);
        } else {
            self.order = (0..self.context.len()).collect();
            self.pos = head;
        }
    }

    pub fn set_repeat(&mut self, repeat: Repeat) {
        self.repeat = repeat;
    }

    /// « Lire ensuite ».
    pub fn play_next(&mut self, id: TrackId) {
        self.enqueue(&[id], true);
    }

    /// « Ajouter à la file ».
    pub fn add_to_queue(&mut self, id: TrackId) {
        self.enqueue(&[id], false);
    }

    /// Ajoute plusieurs titres à la file manuelle en gardant leur ordre (album, playlist) :
    /// en tête avec `next` (« Lire ensuite »), sinon à la fin (« Ajouter à la file »).
    pub fn enqueue(&mut self, ids: &[TrackId], next: bool) {
        if next {
            // `push_front` titre par titre inverserait l'ordre : on part du dernier.
            for &id in ids.iter().rev() {
                self.manual.push_front(id);
            }
        } else {
            self.manual.extend(ids.iter().copied());
        }
    }

    pub fn remove_manual(&mut self, index: usize) -> bool {
        self.manual.remove(index).is_some()
    }

    /// Titre qui suivra naturellement le titre courant (pour le préchargement).
    pub fn peek_next(&self) -> Option<TrackId> {
        self.manual
            .front()
            .copied()
            .or_else(|| self.next_pos().map(|p| self.context[self.order[p]]))
    }

    /// Passe au titre suivant ; `None` en fin de file (l'état reste inchangé).
    pub fn advance(&mut self) -> Option<TrackId> {
        if let Some(id) = self.manual.pop_front() {
            self.push_history();
            self.current = Some(Entry {
                id,
                from_context: false,
            });
            return Some(id);
        }
        let p = self.next_pos()?;
        self.push_history();
        self.pos = p;
        let id = self.context[self.order[p]];
        self.current = Some(Entry {
            id,
            from_context: true,
        });
        Some(id)
    }

    /// Revient au dernier titre de l'historique.
    ///
    /// Un titre courant venant de la file manuelle y est remis en tête ; un titre du
    /// contexte replace le curseur, pour que « Suivant » ramène au titre quitté.
    pub fn previous(&mut self) -> Option<TrackId> {
        let mut entry = self.history.pop()?;
        if let Some(cur) = self.current
            && !cur.from_context
        {
            self.manual.push_front(cur.id);
        }
        if entry.from_context {
            match self.order.iter().position(|&i| self.context[i] == entry.id) {
                Some(p) => self.pos = p,
                // Contexte remplacé depuis : on rejoue le titre sans bouger le curseur.
                None => entry.from_context = false,
            }
        }
        self.current = Some(entry);
        Some(entry.id)
    }

    pub fn manual(&self) -> impl Iterator<Item = TrackId> + '_ {
        self.manual.iter().copied()
    }

    /// Prochains titres du contexte (après le curseur, puis le début si repeat All).
    pub fn upcoming_context(&self, limit: usize) -> Vec<TrackId> {
        let after = self.order.iter().skip(self.pos + 1);
        let wrapped = self.order.iter().take(self.pos + 1);
        let ids = after.map(|&i| self.context[i]);
        if self.repeat == Repeat::All {
            ids.chain(wrapped.map(|&i| self.context[i]))
                .take(limit)
                .collect()
        } else {
            ids.take(limit).collect()
        }
    }

    /// Tous les identifiants référencés (pour persister leurs métadonnées).
    pub fn referenced_ids(&self) -> Vec<TrackId> {
        let mut ids: Vec<TrackId> = self.context.clone();
        ids.extend(self.manual.iter().copied());
        ids.extend(self.history.iter().map(|e| e.id));
        ids.extend(self.current.map(|e| e.id));
        ids.sort_unstable_by_key(|id| id.0);
        ids.dedup();
        ids
    }

    fn next_pos(&self) -> Option<usize> {
        if self.context.is_empty() {
            None
        } else if self.pos + 1 < self.order.len() {
            Some(self.pos + 1)
        } else if self.repeat == Repeat::All {
            Some(0)
        } else {
            None
        }
    }

    fn shuffle_around(&mut self, head: usize, rng: &mut impl Rng) {
        let mut rest: Vec<usize> = (0..self.context.len()).filter(|&i| i != head).collect();
        // `SliceRandom::shuffle` est un Fisher-Yates.
        rest.shuffle(rng);
        self.order = std::iter::once(head).chain(rest).collect();
        self.pos = 0;
    }

    fn push_history(&mut self) {
        if let Some(cur) = self.current {
            self.history.push(cur);
            if self.history.len() > HISTORY_MAX {
                self.history.remove(0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn ids(range: std::ops::Range<u64>) -> Vec<TrackId> {
        range.map(TrackId).collect()
    }

    fn rng() -> StdRng {
        StdRng::seed_from_u64(42)
    }

    fn drain(q: &mut Queue) -> Vec<u64> {
        std::iter::from_fn(|| q.advance()).map(|t| t.0).collect()
    }

    #[test]
    fn plays_context_in_order_then_stops() {
        let mut q = Queue::default();
        assert_eq!(q.set_context(ids(0..4), 1, &mut rng()), Some(TrackId(1)));
        assert_eq!(drain(&mut q), vec![2, 3]);
        assert_eq!(q.current(), Some(TrackId(3)));
        assert_eq!(q.peek_next(), None);
    }

    #[test]
    fn invalid_start_is_rejected() {
        let mut q = Queue::default();
        assert_eq!(q.set_context(ids(0..2), 5, &mut rng()), None);
        assert_eq!(q.set_context(Vec::new(), 0, &mut rng()), None);
        assert_eq!(q.current(), None);
    }

    #[test]
    fn repeat_all_wraps_around() {
        let mut q = Queue::default();
        q.set_context(ids(0..3), 1, &mut rng());
        q.set_repeat(Repeat::All);
        let played: Vec<u64> = (0..5).filter_map(|_| q.advance()).map(|t| t.0).collect();
        assert_eq!(played, vec![2, 0, 1, 2, 0]);
        assert_eq!(
            q.upcoming_context(10),
            ids(1..3).into_iter().chain(ids(0..1)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn repeat_one_is_left_to_mpv() {
        let mut q = Queue::default();
        q.set_context(ids(0..2), 0, &mut rng());
        q.set_repeat(Repeat::One);
        assert_eq!(q.peek_next(), Some(TrackId(1)));
        assert_eq!(q.advance(), Some(TrackId(1)));
    }

    #[test]
    fn manual_queue_has_priority_and_play_next_goes_first() {
        let mut q = Queue::default();
        q.set_context(ids(0..3), 0, &mut rng());
        q.add_to_queue(TrackId(100));
        q.add_to_queue(TrackId(101));
        q.play_next(TrackId(99));
        assert_eq!(q.peek_next(), Some(TrackId(99)));
        assert_eq!(drain(&mut q), vec![99, 100, 101, 1, 2]);
    }

    #[test]
    fn enqueue_next_keeps_album_order_ahead_of_manual_queue() {
        let mut q = Queue::default();
        q.set_context(ids(0..3), 0, &mut rng());
        q.add_to_queue(TrackId(100));
        q.enqueue(&ids(10..13), true);
        assert_eq!(drain(&mut q), vec![10, 11, 12, 100, 1, 2]);
    }

    #[test]
    fn enqueue_appends_album_in_order_after_manual_queue() {
        let mut q = Queue::default();
        q.set_context(ids(0..3), 0, &mut rng());
        q.add_to_queue(TrackId(100));
        q.enqueue(&ids(10..13), false);
        assert_eq!(drain(&mut q), vec![100, 10, 11, 12, 1, 2]);
    }

    #[test]
    fn enqueue_changes_the_preloaded_next_track() {
        // Le titre suivant préchargé dans mpv n'est plus le bon : l'acteur doit le retirer.
        let mut q = Queue::default();
        q.set_context(ids(0..3), 0, &mut rng());
        assert_eq!(q.peek_next(), Some(TrackId(1)));
        q.enqueue(&ids(10..12), false);
        assert_eq!(q.peek_next(), Some(TrackId(10)));
        q.enqueue(&ids(20..22), true);
        assert_eq!(q.peek_next(), Some(TrackId(20)));
        assert_eq!(q.current(), Some(TrackId(0)));
    }

    #[test]
    fn enqueue_nothing_is_a_no_op() {
        let mut q = Queue::default();
        q.set_context(ids(0..2), 0, &mut rng());
        let before = q.clone();
        q.enqueue(&[], true);
        q.enqueue(&[], false);
        assert_eq!(q, before);
    }

    #[test]
    fn remove_manual_entry() {
        let mut q = Queue::default();
        q.set_context(ids(0..2), 0, &mut rng());
        q.add_to_queue(TrackId(100));
        q.add_to_queue(TrackId(101));
        assert!(q.remove_manual(0));
        assert!(!q.remove_manual(5));
        assert_eq!(q.manual().collect::<Vec<_>>(), vec![TrackId(101)]);
    }

    #[test]
    fn shuffle_keeps_current_first_and_is_a_permutation() {
        let mut q = Queue::default();
        q.set_context(ids(0..20), 7, &mut rng());
        q.set_shuffle(true, &mut rng());
        assert_eq!(q.current(), Some(TrackId(7)));
        let mut rest = drain(&mut q);
        assert_eq!(rest.len(), 19);
        assert_ne!(rest, (0..20).filter(|&i| i != 7).collect::<Vec<_>>());
        rest.sort_unstable();
        assert_eq!(rest, (0..20).filter(|&i| i != 7).collect::<Vec<_>>());
    }

    #[test]
    fn unshuffle_restores_original_order_at_current_track() {
        let mut q = Queue::default();
        q.set_context(ids(0..10), 0, &mut rng());
        q.set_shuffle(true, &mut rng());
        q.advance();
        q.advance();
        let current = q.current().map(|t| t.0).unwrap_or_default();
        q.set_shuffle(false, &mut rng());
        assert_eq!(q.current(), Some(TrackId(current)));
        assert_eq!(drain(&mut q), ((current + 1)..10).collect::<Vec<_>>());
    }

    #[test]
    fn shuffled_context_starts_on_requested_track() {
        let mut q = Queue::default();
        q.set_shuffle(true, &mut rng());
        assert_eq!(q.set_context(ids(0..10), 4, &mut rng()), Some(TrackId(4)));
        assert_eq!(drain(&mut q).len(), 9);
    }

    #[test]
    fn previous_then_next_returns_to_left_track() {
        let mut q = Queue::default();
        q.set_context(ids(0..5), 0, &mut rng());
        q.advance();
        q.advance(); // courant : 2
        assert_eq!(q.previous(), Some(TrackId(1)));
        assert_eq!(q.previous(), Some(TrackId(0)));
        assert_eq!(q.previous(), None);
        assert_eq!(q.advance(), Some(TrackId(1)));
        assert_eq!(q.advance(), Some(TrackId(2)));
    }

    #[test]
    fn previous_is_independent_of_shuffle() {
        let mut q = Queue::default();
        q.set_context(ids(0..10), 0, &mut rng());
        q.advance();
        q.advance(); // historique : 0, 1 ; courant : 2
        q.set_shuffle(true, &mut rng());
        assert_eq!(q.previous(), Some(TrackId(1)));
        assert_eq!(q.previous(), Some(TrackId(0)));
    }

    #[test]
    fn previous_from_manual_track_requeues_it() {
        let mut q = Queue::default();
        q.set_context(ids(0..3), 0, &mut rng());
        q.play_next(TrackId(50));
        assert_eq!(q.advance(), Some(TrackId(50)));
        assert_eq!(q.previous(), Some(TrackId(0)));
        assert_eq!(q.peek_next(), Some(TrackId(50)));
    }

    #[test]
    fn previous_into_replaced_context_keeps_cursor() {
        let mut q = Queue::default();
        q.set_context(ids(0..3), 2, &mut rng());
        q.set_context(ids(10..13), 0, &mut rng());
        assert_eq!(q.previous(), Some(TrackId(2)));
        assert_eq!(q.advance(), Some(TrackId(11)));
    }

    #[test]
    fn serde_roundtrip() {
        let mut q = Queue::default();
        q.set_context(ids(0..5), 1, &mut rng());
        q.add_to_queue(TrackId(9));
        q.set_repeat(Repeat::All);
        let json = serde_json::to_string(&q).unwrap();
        let back: Queue = serde_json::from_str(&json).unwrap();
        assert_eq!(back, q);
    }
}
