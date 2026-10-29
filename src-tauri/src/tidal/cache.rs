//! Cache mémoire des métadonnées de titres, alimenté par la recherche et `tracks/{id}`.
//!
//! Permet au lecteur d'afficher la file sans requête réseau ; la copie durable des titres
//! en file vit dans `store` (SQLite).

use std::collections::HashMap;
use std::sync::RwLock;

use super::models::{Track, TrackId};

#[derive(Default)]
pub struct TrackCache {
    tracks: RwLock<HashMap<TrackId, Track>>,
}

impl TrackCache {
    pub fn insert_all<'a>(&self, tracks: impl IntoIterator<Item = &'a Track>) {
        if let Ok(mut map) = self.tracks.write() {
            for t in tracks {
                map.insert(t.id, t.clone());
            }
        }
    }

    pub fn get(&self, id: TrackId) -> Option<Track> {
        self.tracks.read().ok()?.get(&id).cloned()
    }

    /// Titres connus parmi `ids`, dans l'ordre demandé (les inconnus sont ignorés).
    pub fn get_many(&self, ids: impl IntoIterator<Item = TrackId>) -> Vec<Track> {
        let Ok(map) = self.tracks.read() else {
            return Vec::new();
        };
        ids.into_iter()
            .filter_map(|id| map.get(&id).cloned())
            .collect()
    }
}
