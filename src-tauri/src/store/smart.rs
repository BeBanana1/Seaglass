//! Playlists intelligentes locales (#383) : listes calculées en SQL sur l'historique
//! d'écoute de cet appareil (`plays.rs`), sans rien écrire chez TIDAL. Les règles (périodes,
//! seuils) vivent dans `commands::smart_lists` ; ce module ne fait que les requêtes.

use std::collections::HashSet;

use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::stats::CountedTrack;
use super::{Result, Store, StoreError};
use crate::tidal::models::TrackId;

/// Les listes proposées : règles fixes, pas d'éditeur (choix de l'issue #383).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SmartList {
    /// Les plus écoutés sur 30 jours.
    TopMonth,
    /// Écoutés plusieurs fois sur les 7 derniers jours.
    OnRepeat,
    /// Très écoutés, mais plus depuis longtemps.
    Forgotten,
    /// Titres likés absents de l'historique.
    NeverPlayed,
}

impl SmartList {
    /// Clé stable de l'épingle (raccourci Android, lien `seaglass://pin/…`). Le tiret la
    /// distingue d'un id de mix (alphanumérique).
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
    /// Titres écoutés au moins `min_plays` fois, mais pas depuis `before` (ms) : les plus
    /// écoutés d'abord, puis les plus récemment écoutés.
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

    /// Tous les titres présents dans l'historique.
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
        // Titre 1 : 3 écoutes anciennes. Titre 2 : 3 écoutes, dont une récente.
        // Titre 3 : 2 écoutes anciennes, sous le seuil. Titre 4 : 4 écoutes anciennes.
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
        // Les métadonnées enregistrées suivent.
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
        // Depuis 40 : titre 1 trois fois, titre 2 une seule fois, titre 3 une fois.
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
