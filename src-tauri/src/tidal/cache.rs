//! In-memory cache of track metadata, fed by search, lists and
//! `tracks/{id}`.
//!
//! Lets the player display the queue without a network request; the durable copy of the queued
//! tracks lives in `store` (SQLite). Bounded (#90): beyond `CAPACITY` tracks, the least
//! recently used are evicted, except the tracks pinned by the player (those of the
//! queue), which stay whatever size is reached.

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

use super::models::{Track, TrackId};

/// Tracks kept at most (not counting pinned tracks beyond this limit).
const CAPACITY: usize = 20_000;

struct Entry {
    track: Track,
    /// Last use (cache clock), updated on read as well as on write.
    used: AtomicU64,
}

#[derive(Default)]
struct Inner {
    map: HashMap<TrackId, Entry>,
    pinned: HashSet<TrackId>,
}

pub struct TrackCache {
    inner: RwLock<Inner>,
    clock: AtomicU64,
    capacity: usize,
}

impl Default for TrackCache {
    fn default() -> Self {
        Self::with_capacity(CAPACITY)
    }
}

impl TrackCache {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: RwLock::default(),
            clock: AtomicU64::new(0),
            capacity,
        }
    }

    fn tick(&self) -> u64 {
        self.clock.fetch_add(1, Ordering::Relaxed)
    }

    pub fn insert_all<'a>(&self, tracks: impl IntoIterator<Item = &'a Track>) {
        let Ok(mut inner) = self.inner.write() else {
            return;
        };
        for t in tracks {
            let used = AtomicU64::new(self.tick());
            inner.map.insert(
                t.id,
                Entry {
                    track: t.clone(),
                    used,
                },
            );
        }
        if inner.map.len() > self.capacity {
            self.evict(&mut inner);
        }
    }

    /// Goes back under the limit, with a margin of one twentieth so as not to sort the cache on
    /// every insertion. Pinned tracks are never removed.
    fn evict(&self, inner: &mut Inner) {
        let target = self.capacity - self.capacity / 20;
        let excess = inner.map.len().saturating_sub(target);
        let mut candidates: Vec<(u64, TrackId)> = inner
            .map
            .iter()
            .filter(|(id, _)| !inner.pinned.contains(id))
            .map(|(id, e)| (e.used.load(Ordering::Relaxed), *id))
            .collect();
        let count = excess.min(candidates.len());
        if count == 0 {
            return;
        }
        if count < candidates.len() {
            candidates.select_nth_unstable_by_key(count, |&(used, _)| used);
        }
        for (_, id) in &candidates[..count] {
            inner.map.remove(id);
        }
    }

    /// Replaces the set of tracks protected from eviction (those of the queue). Called by
    /// the actor on every queue change: a track that leaves it can be evicted again.
    pub fn set_pinned(&self, ids: impl IntoIterator<Item = TrackId>) {
        if let Ok(mut inner) = self.inner.write() {
            inner.pinned = ids.into_iter().collect();
        }
    }

    pub fn get(&self, id: TrackId) -> Option<Track> {
        let inner = self.inner.read().ok()?;
        let entry = inner.map.get(&id)?;
        entry.used.store(self.tick(), Ordering::Relaxed);
        Some(entry.track.clone())
    }

    /// Known tracks among `ids`, in the requested order (unknown ones are ignored).
    pub fn get_many(&self, ids: impl IntoIterator<Item = TrackId>) -> Vec<Track> {
        let Ok(inner) = self.inner.read() else {
            return Vec::new();
        };
        ids.into_iter()
            .filter_map(|id| {
                let entry = inner.map.get(&id)?;
                entry.used.store(self.tick(), Ordering::Relaxed);
                Some(entry.track.clone())
            })
            .collect()
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.inner.read().map_or(0, |i| i.map.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::AlbumRef;

    fn track(id: u64) -> Track {
        Track {
            id: TrackId(id),
            title: format!("t{id}"),
            version: None,
            duration: 100,
            artists: Vec::new(),
            album: AlbumRef {
                id: 1,
                title: "a".into(),
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

    fn insert(cache: &TrackCache, ids: impl IntoIterator<Item = u64>) {
        let tracks: Vec<Track> = ids.into_iter().map(track).collect();
        cache.insert_all(&tracks);
    }

    #[test]
    fn evicts_least_recently_used_down_to_margin() {
        let cache = TrackCache::with_capacity(100);
        insert(&cache, 0..100);
        assert_eq!(cache.len(), 100);
        // Read recently: survives the eviction.
        assert!(cache.get(TrackId(0)).is_some());
        insert(&cache, [100]);
        // 101 tracks > 100: back to 95.
        assert_eq!(cache.len(), 95);
        assert!(cache.get(TrackId(0)).is_some());
        assert!(cache.get(TrackId(100)).is_some());
        assert!(cache.get(TrackId(1)).is_none());
        assert!(cache.get(TrackId(6)).is_none());
        assert!(cache.get(TrackId(7)).is_some());
    }

    #[test]
    fn pinned_tracks_are_never_evicted() {
        let cache = TrackCache::with_capacity(10);
        insert(&cache, 0..10);
        cache.set_pinned((0..5).map(TrackId));
        insert(&cache, 10..30);
        for id in 0..5 {
            assert!(
                cache.get(TrackId(id)).is_some(),
                "pinned track {id} evicted"
            );
        }
        assert!(cache.len() <= 10);
        // More pinned than the capacity: the cache exceeds the limit rather than lose them.
        cache.set_pinned((100..120).map(TrackId));
        insert(&cache, 100..120);
        assert_eq!(cache.get_many((100..120).map(TrackId)).len(), 20);
        // Once unpinned, they can be evicted again.
        cache.set_pinned([]);
        insert(&cache, [200]);
        assert!(cache.len() <= 10);
    }
}
