//! Track availability (#383): a track removed from the catalog (or no longer playable in the
//! account's country) has `streamReady` or `allowStreaming` at `false`, or `tracks/{id}` answers
//! 404. `Track` does not keep this information: it is remembered here for the session,
//! filled in along the way by liked tracks (`favorite_tracks`) and, on demand, by
//! `tracks/{id}` (smart playlists, whose tracks come from the local history).

use std::collections::HashMap;
use std::sync::Mutex;

use super::raw::RawTrack;
use super::{TidalClient, TidalError};
use crate::tidal::models::TrackId;

/// Tracks remembered at most; beyond that, the table starts again from zero (only a cache).
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
    /// Is the track playable? Already known: no request. Otherwise `tracks/{id}`, whose
    /// track also goes into the cache. `None` if unknown (network error): up to the caller
    /// not to hide anything.
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
                tracing::debug!("availability of track {id} unknown: {e}");
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
