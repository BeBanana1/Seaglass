//! Local smart playlists (#383): lists computed in SQL over this device's listening
//! history (`plays.rs`), without writing anything to TIDAL. The rules (periods,
//! thresholds) live in `commands::smart_lists`; this module only runs the queries.

use std::collections::HashSet;

use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::stats::CountedTrack;
use super::{Result, Store, StoreError};
use crate::tidal::models::TrackId;

/// Local smart playlist (#383), computed by the backend from this device's history (and the
/// liked tracks for `neverPlayed`). The lists offered: fixed rules, no editor (decided in #383).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export, export_to = "../../src/lib/bindings/")
)]
pub enum SmartList {
    /// Most played over 30 days.
    TopMonth,
    /// Played several times over the last 7 days.
    OnRepeat,
    /// Played a lot, but not for a long time.
    Forgotten,
    /// Liked tracks missing from the history.
    NeverPlayed,
}

impl SmartList {
    /// Stable key of the pin (Android shortcut, `seaglass://pin/…` link). The dash tells
    /// it apart from a mix id (alphanumeric).
    pub fn key(self) -> &'static str {
        match self {
            Self::TopMonth => "smart-top-month",
            Self::OnRepeat => "smart-on-repeat",
            Self::Forgotten => "smart-forgotten",
            Self::NeverPlayed => "smart-never-played",
        }
    }
}

impl Store {
    /// Tracks played at least `min_plays` times, but not since `before` (ms): the most
    /// played first, then the most recently played.
    pub fn forgotten_tracks(
        &self,
        before: i64,
        min_plays: i64,
        limit: usize,
    ) -> Result<Vec<CountedTrack>> {
        self.counted_tracks(
            "SELECT p.track_id, t.json, COUNT(*) AS n, MAX(p.played_at) AS last FROM plays p
             LEFT JOIN played_tracks t ON t.id = p.track_id
             GROUP BY p.track_id HAVING n >= ?2 AND last < ?1
             ORDER BY n DESC, last DESC LIMIT ?3",
            params![before, min_plays, limit as i64],
        )
    }

    /// All the tracks in the history.
    pub fn played_track_ids(&self) -> Result<HashSet<TrackId>> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let mut stmt = db.conn.prepare("SELECT DISTINCT track_id FROM plays")?;
        let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
        Ok(rows
            .map(|id| id.map(|id| TrackId(id as u64)))
            .collect::<rusqlite::Result<_>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::super::plays::tests::play;
    use super::super::stats::Span;
    use super::*;

    fn ids(tracks: Vec<CountedTrack>) -> Vec<(u64, i64)> {
        tracks.into_iter().map(|(id, _, n)| (id.0, n)).collect()
    }

    #[test]
    fn forgotten_tracks_were_played_often_but_not_lately() {
        let store = Store::in_memory().unwrap();
        // Track 1: 3 old plays. Track 2: 3 plays, one of them recent.
        // Track 3: 2 old plays, under the threshold. Track 4: 4 old plays.
        for (t, at) in [(1, 10), (1, 11), (1, 12), (2, 10), (2, 11), (2, 500)] {
            play(&store, t, 10, at);
        }
        for (t, at) in [(3, 10), (3, 11), (4, 1), (4, 2), (4, 3), (4, 4)] {
            play(&store, t, 10, at);
        }
        assert_eq!(
            ids(store.forgotten_tracks(100, 3, 10).unwrap()),
            [(4, 4), (1, 3)]
        );
        assert_eq!(ids(store.forgotten_tracks(100, 3, 1).unwrap()), [(4, 4)]);
        // The recorded metadata follows.
        let found = store.forgotten_tracks(100, 3, 10).unwrap();
        assert_eq!(
            found[0].1.as_ref().map(|t| t.title.as_str()),
            Some("Titre 4")
        );
    }

    #[test]
    fn on_repeat_needs_several_recent_plays() {
        let store = Store::in_memory().unwrap();
        for (t, at) in [(1, 50), (1, 60), (1, 70), (2, 1), (2, 2), (2, 60), (3, 80)] {
            play(&store, t, 10, at);
        }
        // Since 40: track 1 three times, track 2 only once, track 3 once.
        assert_eq!(
            ids(store.top_tracks(Span::since(40), 2, 10).unwrap()),
            [(1, 3)]
        );
        assert_eq!(
            ids(store.top_tracks(Span::since(40), 1, 10).unwrap()),
            [(1, 3), (3, 1), (2, 1)]
        );
    }

    #[test]
    fn played_ids() {
        let store = Store::in_memory().unwrap();
        assert!(store.played_track_ids().unwrap().is_empty());
        play(&store, 1, 10, 1);
        play(&store, 1, 10, 2);
        play(&store, 7, 10, 3);
        assert_eq!(
            store.played_track_ids().unwrap(),
            HashSet::from([TrackId(1), TrackId(7)])
        );
    }

    #[test]
    fn keys_are_distinct() {
        let all = [
            SmartList::TopMonth,
            SmartList::OnRepeat,
            SmartList::Forgotten,
            SmartList::NeverPlayed,
        ];
        let keys: HashSet<&str> = all.iter().map(|l| l.key()).collect();
        assert_eq!(keys.len(), all.len());
    }
}
