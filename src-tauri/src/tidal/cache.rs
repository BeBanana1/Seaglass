//! Cache mémoire des métadonnées de titres, alimenté par la recherche, les listes et
//! `tracks/{id}`.
//!
//! Permet au lecteur d'afficher la file sans requête réseau ; la copie durable des titres
//! en file vit dans `store` (SQLite). Borné (#90) : au-delà de `CAPACITY` titres, les moins
//! récemment utilisés sont évincés, sauf les titres épinglés par le lecteur (ceux de la
//! file), qui restent quelle que soit la taille atteinte.

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

use super::models::{Track, TrackId};

/// Titres gardés au plus (hors titres épinglés au-delà de cette limite).
const CAPACITY: usize = 20_000;

struct Entry {
    track: Track,
    /// Dernier usage (horloge du cache), mis à jour à la lecture comme à l'écriture.
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

    /// Redescend sous la limite, avec une marge d'un vingtième pour ne pas trier le cache à
    /// chaque insertion. Les titres épinglés ne sont jamais retirés.
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

    /// Remplace l'ensemble des titres protégés de l'éviction (ceux de la file). Appelé par
    /// l'acteur à chaque changement de file : un titre qui en sort redevient évinçable.
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

    /// Titres connus parmi `ids`, dans l'ordre demandé (les inconnus sont ignorés).
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
        // Lu récemment : survit à l'éviction.
        assert!(cache.get(TrackId(0)).is_some());
        insert(&cache, [100]);
        // 101 titres > 100 : retour à 95.
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
                "titre épinglé {id} évincé"
            );
        }
        assert!(cache.len() <= 10);
        // Plus d'épinglés que la capacité : le cache dépasse la limite plutôt que de les perdre.
        cache.set_pinned((100..120).map(TrackId));
        insert(&cache, 100..120);
        assert_eq!(cache.get_many((100..120).map(TrackId)).len(), 20);
        // Une fois désépinglés, ils redeviennent évinçables.
        cache.set_pinned([]);
        insert(&cache, [200]);
        assert!(cache.len() <= 10);
    }
}
