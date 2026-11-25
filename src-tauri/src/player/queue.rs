//! Queue: pure logic, with no mpv or network.
//!
//! - `context`: tracks of the original album / playlist / search;
//!   `order`: play order (indices into `context`); `pos`: cursor in `order`.
//! - `manual`: manual queue, takes priority over the context; Previous also puts back at
//!   its head the track left when the cursor cannot bring it back (see `Entry`).
//! - `history`: previous tracks, independent of shuffle.
//! - `Repeat::One` is handled by mpv (`loop-file=inf`): the queue ignores it.

use std::collections::{HashSet, VecDeque};

use rand::Rng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use super::source::PlaySource;
use crate::tidal::models::TrackId;
use entry::{Entry, Origin};

const HISTORY_MAX: usize = 500;
/// Recent history tracks kept out of an automatic radio (#78).
const RADIO_RECENT: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export, export_to = "../../src/lib/bindings/")
)]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

/// Part of the queue shown by the UI (`QueueView`), for "Play from the queue" (#167).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueuePart {
    /// `QueueView::manual`: the manual queue.
    Manual,
    /// `QueueView::upcoming`: the rest of the context (`upcoming_context`).
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
    /// Track whose radio extends the exhausted queue (#78); cleared by a new context.
    #[serde(default)]
    radio: Option<TrackId>,
    /// Where the context comes from (#124); cleared with it.
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

    /// Replaces the context and starts on `start`. The manual queue is kept, except the
    /// tracks recalled by Previous.
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
        // The tracks recalled by Previous belonged to the previous navigation.
        self.manual.retain(|e| e.queued_by_user());
        self.current = Some(Entry::new(id, Origin::Context));
        Some(id)
    }

    /// On: Fisher-Yates over the other tracks, current track first.
    /// Off: original order, cursor put back on the current track.
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

    /// "Play next".
    pub fn play_next(&mut self, id: TrackId) {
        self.enqueue(&[id], true);
    }

    /// "Add to queue".
    pub fn add_to_queue(&mut self, id: TrackId) {
        self.enqueue(&[id], false);
    }

    /// Adds several tracks to the manual queue keeping their order (album, playlist): at
    /// the head with `next` ("Play next"), otherwise at the end ("Add to queue").
    pub fn enqueue(&mut self, ids: &[TrackId], next: bool) {
        if next {
            // `push_front` track by track would reverse the order: start from the last one.
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

    /// Removes from the manual queue one entry per given identifier, the first one found
    /// (Jam guests removing their additions, #223). Returns the number removed.
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

    /// Moves the manual queue entry `from` so that it ends at index `to` (drag and drop,
    /// Move up / Move down). Returns `false` if nothing changes.
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

    /// Empties the manual queue and the rest of the context, without touching the current
    /// track or the history. The current track is detached from the context: nothing follows
    /// it anymore, even in repeat All. Returns `false` if there was nothing to empty.
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

    /// Track whose radio extends the queue, if there is one (UI separator).
    pub fn radio(&self) -> Option<TrackId> {
        self.radio
    }

    /// Source of the context, only if the current track comes from it (not a manual queue
    /// track, nor a history track whose context was replaced).
    pub fn source(&self) -> Option<&PlaySource> {
        self.current
            .filter(|c| c.in_context())
            .and(self.source.as_ref())
    }

    /// Starting track of the automatic radio when the current track comes from it (#78, #124).
    pub fn radio_playing(&self) -> Option<TrackId> {
        let cur = self.current.filter(|c| c.in_context())?;
        self.radio.filter(|&seed| seed != cur.id)
    }

    /// A radio may extend the queue: nothing follows the current track, without repeat
    /// (repeat All takes priority, repeat One is handled by mpv).
    pub fn wants_radio(&self) -> bool {
        self.repeat == Repeat::Off && self.current.is_some() && self.peek_next().is_none()
    }

    /// Extends an exhausted queue with the radio of `seed`, the current track (#78).
    ///
    /// Refused if the current track changed or a next one already exists (queue changed
    /// during the request). The current track, recent history tracks and duplicates are
    /// discarded. The new context starts with the current track, cursor on it: Next moves
    /// to the first track of the radio.
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

    /// Track that will naturally follow the current one (for the preload).
    pub fn peek_next(&self) -> Option<TrackId> {
        self.manual
            .front()
            .map(|e| e.id)
            .or_else(|| self.next_pos().map(|p| self.context[self.order[p]]))
    }

    /// Nothing left to play after the current track without looping (repeat All) or radio:
    /// empty manual queue and cursor on the last track of the play order (sleep timer, #436).
    pub fn at_end(&self) -> bool {
        self.manual.is_empty() && self.pos + 1 >= self.order.len()
    }

    /// Moves to the next track; `None` at the end of the queue (the state stays unchanged).
    pub fn advance(&mut self) -> Option<TrackId> {
        if let Some(mut entry) = self.manual.pop_front() {
            self.push_history();
            // Context track recalled by Previous: the cursor did not move and still points
            // to it, unless the context changed since.
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

    /// Plays the track `index` of the part `part` as the UI shows it (#167), if it is still
    /// `id` (otherwise the queue changed since it was shown: `None`, nothing moves).
    ///
    /// - Manual queue: the tracks before it are removed, those after it stay.
    /// - Rest of the context: the cursor jumps to it, in the current play order (repeat
    ///   All included); the manual queue is kept and comes right after it.
    ///
    /// Skipped tracks do not go into the history; the track left does.
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

    /// Position in `order` of the track `offset` of `upcoming_context`.
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

    /// Goes back to the last track of the history.
    ///
    /// A context track moves the cursor back, so that Next returns to the track left.
    /// Otherwise, the track left is put back at the head of the manual queue: in its place if
    /// it came from the queue, recalled (discarded by a new context) if from elsewhere (#166).
    pub fn previous(&mut self) -> Option<TrackId> {
        let mut entry = self.history.pop()?;
        if entry.in_context() {
            match self.order.iter().position(|&i| self.context[i] == entry.id) {
                Some(p) => self.pos = p,
                // Context replaced since: the track is replayed without moving the cursor.
                None => entry.origin = Origin::Detached,
            }
        }
        if let Some(cur) = self.current {
            // Between two context tracks, the cursor is enough to return to the track left.
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

    /// Next tracks of the context (after the cursor, then the start if repeat All).
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

    /// Full play order (#327): current track, manual queue, then the rest of the context,
    /// at most `limit` tracks.
    pub fn play_order(&self, limit: usize) -> Vec<TrackId> {
        let mut ids: Vec<TrackId> = self.current().into_iter().chain(self.manual()).collect();
        ids.truncate(limit);
        let rest = limit - ids.len();
        ids.extend(self.upcoming_context(rest));
        ids
    }

    /// Every referenced identifier (to persist their metadata).
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
        // `SliceRandom::shuffle` is a Fisher-Yates.
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

mod check;
mod entry;
#[cfg(test)]
mod tests;
