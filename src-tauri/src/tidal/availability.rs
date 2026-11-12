//! Disponibilité des titres (#383) : un titre retiré du catalogue (ou plus lisible dans le
//! pays du compte) a `streamReady` ou `allowStreaming` à `false`, ou `tracks/{id}` répond
//! 404. `Track` ne garde pas cette information : elle est retenue ici pour la session,
//! remplie au passage par les titres likés (`favorite_tracks`) et, à la demande, par
//! `tracks/{id}` (playlists intelligentes, dont les titres viennent de l'historique local).

use std::collections::HashMap;
use std::sync::Mutex;

use super::raw::RawTrack;
use super::{TidalClient, TidalError};
use crate::tidal::models::TrackId;

/// Titres retenus au plus ; au-delà, la table repart de zéro (ce n'est qu'un cache).
const CAPACITY: usize = 50_000;

#[derive(Default)]
pub struct Availability {
    known: Mutex<HashMap<TrackId, bool>>,
}

impl Availability {
    pub(super) fn record(&self, entries: impl IntoIterator<Item = (TrackId, bool)>) {
        if let Ok(mut known) = self.known.lock() {
            if known.len() > CAPACITY {
                known.clear();
            }
            known.extend(entries);
        }
    }

    fn get(&self, id: TrackId) -> Option<bool> {
        self.known.lock().ok()?.get(&id).copied()
    }

    pub fn clear(&self) {
        if let Ok(mut known) = self.known.lock() {
            known.clear();
        }
    }
}

impl TidalClient {
    /// Le titre est-il lisible ? Déjà connu : sans requête. Sinon `tracks/{id}`, dont le
    /// titre va aussi dans le cache. `None` si on ne sait pas (erreur réseau) : à l'appelant
    /// de ne rien masquer.
    pub async fn is_available(&self, id: TrackId) -> Option<bool> {
        if let Some(known) = self.availability.get(id) {
            return Some(known);
        }
        let available = match self.get::<RawTrack>(&format!("tracks/{id}"), &[]).await {
            Ok(raw) => {
                let available = raw.streamable();
                if let Some(track) = raw.into_track() {
                    self.cache.insert_all([&track]);
                }
                available
            }
            Err(TidalError::Api { status: 404, .. }) => false,
            Err(e) => {
                tracing::debug!("disponibilité du titre {id} inconnue : {e}");
                return None;
            }
        };
        self.availability.record([(id, available)]);
        Some(available)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_clears() {
        let a = Availability::default();
        assert_eq!(a.get(TrackId(1)), None);
        a.record([(TrackId(1), true), (TrackId(2), false)]);
        assert_eq!(a.get(TrackId(1)), Some(true));
        assert_eq!(a.get(TrackId(2)), Some(false));
        a.clear();
        assert_eq!(a.get(TrackId(2)), None);
    }
}
