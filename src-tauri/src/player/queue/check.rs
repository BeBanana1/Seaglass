//! Queue received from another device (playback handoff, #414): checked before being given
//! to the actor, which indexes `context[order[pos]]` without checking again.

use std::collections::HashSet;

use super::{HISTORY_MAX, Queue};

/// Most context tracks accepted (a 10,000-track playlist goes through).
const MAX_CONTEXT: usize = 20_000;
/// Most manual-queue tracks accepted.
const MAX_MANUAL: usize = 2_000;

impl Queue {
    /// The queue if it is consistent and has a current track, otherwise `None`. The history
    /// is cut down to its usual size and the "Played from" source cleaned (`is_uuid` comes
    /// from the commands, which validate the input).
    pub fn received(mut self, is_uuid: impl Fn(&str) -> bool) -> Option<Self> {
        self.current?;
        if self.context.len() > MAX_CONTEXT || self.manual.len() > MAX_MANUAL {
            return None;
        }
        if self.order.len() != self.context.len() {
            return None;
        }
        let mut seen = HashSet::with_capacity(self.order.len());
        if !self
            .order
            .iter()
            .all(|&i| i < self.context.len() && seen.insert(i))
        {
            return None;
        }
        // Cursor in `order`, or 0 for an empty context.
        if self.pos >= self.order.len().max(1) {
            return None;
        }
        if self.history.len() > HISTORY_MAX {
            let extra = self.history.len() - HISTORY_MAX;
            self.history.drain(..extra);
        }
        self.source = self.source.take().and_then(|s| s.sanitize(&is_uuid));
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    use super::*;
    use crate::tidal::models::TrackId;

    fn ids(range: std::ops::Range<u64>) -> Vec<TrackId> {
        range.map(TrackId).collect()
    }

    fn playing() -> Queue {
        let mut rng = StdRng::seed_from_u64(1);
        let mut q = Queue::default();
        q.set_context(ids(1..6), 2, None, &mut rng);
        q.add_to_queue(TrackId(42));
        q
    }

    fn any_uuid(_: &str) -> bool {
        true
    }

    #[test]
    fn a_consistent_queue_is_accepted_unchanged() {
        let q = playing();
        assert_eq!(q.clone().received(any_uuid), Some(q));
    }

    #[test]
    fn a_shuffled_queue_survives_the_round_trip() {
        let mut rng = StdRng::seed_from_u64(7);
        let mut q = playing();
        q.set_shuffle(true, &mut rng);
        let json = serde_json::to_string(&q).unwrap();
        let back: Queue = serde_json::from_str(&json).unwrap();
        assert_eq!(back.received(any_uuid), Some(q));
    }

    #[test]
    fn nothing_playing_is_refused() {
        assert_eq!(Queue::default().received(any_uuid), None);
    }

    #[test]
    fn a_broken_order_is_refused() {
        let mut q = playing();
        q.order[0] = 99;
        assert_eq!(q.received(any_uuid), None);

        let mut q = playing();
        q.order[1] = q.order[0];
        assert_eq!(q.received(any_uuid), None);

        let mut q = playing();
        q.order.pop();
        assert_eq!(q.received(any_uuid), None);

        let mut q = playing();
        q.pos = q.order.len();
        assert_eq!(q.received(any_uuid), None);
    }

    #[test]
    fn oversized_queues_are_refused_and_history_trimmed() {
        let mut q = playing();
        q.context = ids(0..MAX_CONTEXT as u64 + 1);
        q.order = (0..q.context.len()).collect();
        assert_eq!(q.received(any_uuid), None);

        let mut q = playing();
        for i in 0..HISTORY_MAX as u64 + 10 {
            q.history.push(super::super::Entry::new(
                TrackId(i),
                super::super::Origin::Detached,
            ));
        }
        let q = q.received(any_uuid).unwrap();
        assert_eq!(q.history.len(), HISTORY_MAX);
        assert_eq!(
            q.history.last().map(|e| e.id),
            Some(TrackId(HISTORY_MAX as u64 + 9))
        );
    }

    #[test]
    fn a_bad_source_is_dropped() {
        let mut q = playing();
        q.source = Some(crate::player::PlaySource::Playlist {
            uuid: "pas-un-uuid".into(),
            title: "Mix".into(),
        });
        let q = q.received(|_| false).unwrap();
        assert_eq!(q.source, None);
    }
}
