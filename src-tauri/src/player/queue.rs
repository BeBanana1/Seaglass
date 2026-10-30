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

    /// Déplace l'entrée `from` de la file manuelle pour qu'elle finisse à l'index `to`
    /// (glisser-déposer, Monter / Descendre). Renvoie `false` si rien ne change.
    pub fn move_manual(&mut self, from: usize, to: usize) -> bool {
        let len = self.manual.len();
        if from == to || from >= len || to >= len {
            return false;
        }
        if let Some(id) = self.manual.remove(from) {
            self.manual.insert(to, id);
        }
        true
    }

    /// Vide la file manuelle et la suite du contexte, sans toucher au titre courant ni à
    /// l'historique. Le titre courant est détaché du contexte : rien ne le suit plus,
    /// même en repeat All. Renvoie `false` s'il n'y avait rien à vider.
    pub fn clear_upcoming(&mut self) -> bool {
        if self.manual.is_empty() && self.peek_next().is_none() {
            return false;
        }
        self.manual.clear();
        self.context.clear();
        self.order.clear();
        self.pos = 0;
        if let Some(cur) = &mut self.current {
            cur.from_context = false;
        }
        true
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
mod tests;
