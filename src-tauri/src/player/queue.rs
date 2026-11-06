//! File d'attente : logique pure, sans mpv ni réseau.
//!
//! - `context` : titres de l'album / playlist / recherche d'origine ;
//!   `order` : ordre de lecture (indices dans `context`) ; `pos` : curseur dans `order`.
//! - `manual` : file manuelle, prioritaire sur le contexte ; « Précédent » y remet aussi en
//!   tête le titre quitté quand le curseur ne peut pas y ramener (voir `Entry`).
//! - `history` : titres précédents, indépendant du shuffle.
//! - `Repeat::One` est géré par mpv (`loop-file=inf`) : la file l'ignore.

use std::collections::{HashSet, VecDeque};

use rand::Rng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use super::source::PlaySource;
use crate::tidal::models::TrackId;
use entry::{Entry, Origin};

const HISTORY_MAX: usize = 500;
/// Titres récents de l'historique écartés d'une radio automatique (#78).
const RADIO_RECENT: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

/// Partie de la file affichée par l'UI (`QueueView`), pour « Lire depuis la file » (#167).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueuePart {
    /// `QueueView::manual` : la file manuelle.
    Manual,
    /// `QueueView::upcoming` : la suite du contexte (`upcoming_context`).
    Upcoming,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Queue {
    context: Vec<TrackId>,
    order: Vec<usize>,
    pos: usize,
    manual: VecDeque<Entry>,
    history: Vec<Entry>,
    current: Option<Entry>,
    repeat: Repeat,
    shuffle: bool,
    /// Titre dont la radio prolonge la file épuisée (#78) ; effacé par un nouveau contexte.
    #[serde(default)]
    radio: Option<TrackId>,
    /// D'où vient le contexte (#124) ; effacé avec lui.
    #[serde(default)]
    source: Option<PlaySource>,
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

    /// Remplace le contexte et démarre sur `start`. La file manuelle est conservée, sauf les
    /// titres rappelés par « Précédent ».
    pub fn set_context(
        &mut self,
        tracks: Vec<TrackId>,
        start: usize,
        source: Option<PlaySource>,
        rng: &mut impl Rng,
    ) -> Option<TrackId> {
        let id = *tracks.get(start)?;
        self.push_history();
        self.radio = None;
        self.source = source;
        self.order = (0..tracks.len()).collect();
        self.context = tracks;
        if self.shuffle {
            self.shuffle_around(start, rng);
        } else {
            self.pos = start;
        }
        // Les titres rappelés par « Précédent » appartenaient à la navigation précédente.
        self.manual.retain(|e| e.queued_by_user());
        self.current = Some(Entry::new(id, Origin::Context));
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
                self.manual.push_front(Entry::new(id, Origin::Manual));
            }
        } else {
            self.manual
                .extend(ids.iter().map(|&id| Entry::new(id, Origin::Manual)));
        }
    }

    pub fn remove_manual(&mut self, index: usize) -> bool {
        self.manual.remove(index).is_some()
    }

    /// Retire de la file manuelle une entrée par identifiant donné, la première trouvée
    /// (invités de la Jam qui retirent leurs ajouts, #223). Renvoie le nombre retiré.
    pub fn remove_manual_ids(&mut self, ids: &[TrackId]) -> usize {
        let mut removed = 0;
        for id in ids {
            if let Some(i) = self.manual.iter().position(|e| e.id == *id) {
                self.manual.remove(i);
                removed += 1;
            }
        }
        removed
    }

    /// Déplace l'entrée `from` de la file manuelle pour qu'elle finisse à l'index `to`
    /// (glisser-déposer, Monter / Descendre). Renvoie `false` si rien ne change.
    pub fn move_manual(&mut self, from: usize, to: usize) -> bool {
        let len = self.manual.len();
        if from == to || from >= len || to >= len {
            return false;
        }
        if let Some(entry) = self.manual.remove(from) {
            self.manual.insert(to, entry);
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
        self.radio = None;
        self.source = None;
        if let Some(cur) = &mut self.current {
            cur.origin = Origin::Detached;
        }
        true
    }

    /// Titre dont la radio prolonge la file, s'il y en a une (séparateur de l'UI).
    pub fn radio(&self) -> Option<TrackId> {
        self.radio
    }

    /// Source du contexte, seulement si le titre courant en vient (pas d'un titre de la file
    /// manuelle, ni d'un titre de l'historique dont le contexte a été remplacé).
    pub fn source(&self) -> Option<&PlaySource> {
        self.current
            .filter(|c| c.in_context())
            .and(self.source.as_ref())
    }

    /// Titre de départ de la radio automatique quand le titre courant en vient (#78, #124).
    pub fn radio_playing(&self) -> Option<TrackId> {
        let cur = self.current.filter(|c| c.in_context())?;
        self.radio.filter(|&seed| seed != cur.id)
    }

    /// Une radio peut prolonger la file : rien ne suit le titre courant, sans répétition
    /// (repeat All est prioritaire, repeat One est géré par mpv).
    pub fn wants_radio(&self) -> bool {
        self.repeat == Repeat::Off && self.current.is_some() && self.peek_next().is_none()
    }

    /// Prolonge une file épuisée par la radio de `seed`, le titre courant (#78).
    ///
    /// Refusé si le titre courant a changé ou si un suivant existe déjà (file modifiée
    /// pendant la requête). Le titre courant, les titres récents de l'historique et les
    /// doublons sont écartés. Le nouveau contexte commence par le titre courant, curseur
    /// dessus : « Suivant » passe au premier titre de la radio.
    pub fn extend_with_radio(&mut self, seed: TrackId, tracks: &[TrackId]) -> bool {
        if self.current() != Some(seed) || !self.wants_radio() {
            return false;
        }
        let mut seen: HashSet<TrackId> = self
            .history
            .iter()
            .rev()
            .take(RADIO_RECENT)
            .map(|e| e.id)
            .collect();
        seen.insert(seed);
        let fresh: Vec<TrackId> = tracks
            .iter()
            .copied()
            .filter(|&id| seen.insert(id))
            .collect();
        if fresh.is_empty() {
            return false;
        }
        self.context = std::iter::once(seed).chain(fresh).collect();
        self.order = (0..self.context.len()).collect();
        self.pos = 0;
        self.radio = Some(seed);
        self.source = None;
        if let Some(cur) = &mut self.current {
            cur.origin = Origin::Context;
        }
        true
    }

    /// Titre qui suivra naturellement le titre courant (pour le préchargement).
    pub fn peek_next(&self) -> Option<TrackId> {
        self.manual
            .front()
            .map(|e| e.id)
            .or_else(|| self.next_pos().map(|p| self.context[self.order[p]]))
    }

    /// Passe au titre suivant ; `None` en fin de file (l'état reste inchangé).
    pub fn advance(&mut self) -> Option<TrackId> {
        if let Some(mut entry) = self.manual.pop_front() {
            self.push_history();
            // Titre du contexte rappelé par « Précédent » : le curseur n'a pas bougé et le
            // désigne encore, sauf si le contexte a changé depuis.
            if entry.in_context() && self.context_at_cursor() != Some(entry.id) {
                entry.origin = Origin::Detached;
            }
            self.current = Some(entry);
            return Some(entry.id);
        }
        let p = self.next_pos()?;
        self.push_history();
        self.pos = p;
        let id = self.context[self.order[p]];
        self.current = Some(Entry::new(id, Origin::Context));
        Some(id)
    }

    /// Lit le titre `index` de la partie `part` telle que l'UI l'affiche (#167), s'il s'agit
    /// toujours de `id` (sinon la file a changé depuis l'affichage : `None`, rien ne bouge).
    ///
    /// - File manuelle : les titres avant lui sont retirés, ceux d'après restent.
    /// - Suite du contexte : le curseur saute dessus, dans l'ordre de lecture courant (repeat
    ///   All compris) ; la file manuelle est gardée et passe juste après lui.
    ///
    /// Les titres sautés ne vont pas dans l'historique ; le titre quitté, si.
    pub fn jump_to(&mut self, part: QueuePart, index: usize, id: TrackId) -> Option<TrackId> {
        match part {
            QueuePart::Manual => {
                if self.manual.get(index)?.id != id {
                    return None;
                }
                self.manual.drain(..index);
                self.advance()
            }
            QueuePart::Upcoming => {
                let p = self.upcoming_pos(index)?;
                if self.context[self.order[p]] != id {
                    return None;
                }
                self.push_history();
                self.pos = p;
                self.current = Some(Entry::new(id, Origin::Context));
                Some(id)
            }
        }
    }

    /// Position dans `order` du titre `offset` de `upcoming_context`.
    fn upcoming_pos(&self, offset: usize) -> Option<usize> {
        let len = self.order.len();
        let p = self.pos.checked_add(1)?.checked_add(offset)?;
        if p < len {
            Some(p)
        } else if self.repeat == Repeat::All && p - len <= self.pos {
            Some(p - len)
        } else {
            None
        }
    }

    /// Revient au dernier titre de l'historique.
    ///
    /// Un titre du contexte replace le curseur, pour que « Suivant » ramène au titre quitté.
    /// Sinon, le titre quitté est remis en tête de la file manuelle : à sa place s'il venait
    /// de la file, rappelé (écarté par un nouveau contexte) s'il venait d'ailleurs (#166).
    pub fn previous(&mut self) -> Option<TrackId> {
        let mut entry = self.history.pop()?;
        if entry.in_context() {
            match self.order.iter().position(|&i| self.context[i] == entry.id) {
                Some(p) => self.pos = p,
                // Contexte remplacé depuis : on rejoue le titre sans bouger le curseur.
                None => entry.origin = Origin::Detached,
            }
        }
        if let Some(cur) = self.current {
            // Entre deux titres du contexte, le curseur suffit à ramener au titre quitté.
            let cursor_returns = cur.in_context() && entry.in_context();
            if !cursor_returns {
                self.manual.push_front(cur);
            }
        }
        self.current = Some(entry);
        Some(entry.id)
    }

    pub fn manual(&self) -> impl Iterator<Item = TrackId> + '_ {
        self.manual.iter().map(|e| e.id)
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
        ids.extend(self.manual.iter().map(|e| e.id));
        ids.extend(self.history.iter().map(|e| e.id));
        ids.extend(self.current.map(|e| e.id));
        ids.sort_unstable_by_key(|id| id.0);
        ids.dedup();
        ids
    }

    fn context_at_cursor(&self) -> Option<TrackId> {
        self.order.get(self.pos).map(|&i| self.context[i])
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

mod entry;
#[cfg(test)]
mod tests;
