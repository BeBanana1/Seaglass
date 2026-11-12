//! Historique d'écoute local (#202) : rien n'est envoyé à TIDAL ni ailleurs.
//!
//! Une ligne par écoute comptée (règle de `player::listen`), les `MAX_PLAYS` plus récentes
//! seulement. L'album est gardé en JSON (`AlbumSummary`, de quoi afficher une carte sans
//! requête), la source (`PlaySource`, #124) aussi, pour un usage futur.

use rusqlite::params;

use super::{Result, Store, StoreError};
use crate::player::source::PlaySource;
use crate::tidal::models::{AlbumSummary, TrackId};

/// Écoutes gardées au plus.
pub const MAX_PLAYS: i64 = 5_000;

pub(super) const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS plays (
        id        INTEGER PRIMARY KEY AUTOINCREMENT,
        track_id  INTEGER NOT NULL,
        album_id  INTEGER,
        album     TEXT,
        source    TEXT,
        played_at INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS plays_album ON plays (album_id, id);
";

/// Une écoute à enregistrer.
pub struct Play<'a> {
    pub track: TrackId,
    pub album: Option<&'a AlbumSummary>,
    pub source: Option<&'a PlaySource>,
    /// Millisecondes depuis l'epoch Unix.
    pub played_at: i64,
}

impl Store {
    /// Ajoute une écoute et élague au-delà de `MAX_PLAYS`.
    pub fn add_play(&self, play: &Play) -> Result<()> {
        let album = play.album.map(serde_json::to_string).transpose()?;
        let source = play.source.map(serde_json::to_string).transpose()?;
        let mut db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let tx = db.conn.transaction()?;
        tx.execute(
            "INSERT INTO plays (track_id, album_id, album, source, played_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                play.track.0 as i64,
                play.album.map(|a| a.id as i64),
                album,
                source,
                play.played_at
            ],
        )?;
        tx.execute(
            "DELETE FROM plays WHERE id <= (SELECT MAX(id) FROM plays) - ?1",
            [MAX_PLAYS],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Albums écoutés, du plus récent au plus ancien, sans doublon.
    pub fn recent_albums(&self, limit: usize) -> Result<Vec<AlbumSummary>> {
        self.albums(
            "SELECT album, MAX(id) AS last FROM plays WHERE album_id IS NOT NULL
             GROUP BY album_id ORDER BY last DESC LIMIT ?1",
            params![limit as i64],
        )
    }

    /// Albums les plus écoutés depuis `since` (ms), avec au moins `min_plays` écoutes.
    pub fn top_albums(
        &self,
        since: i64,
        min_plays: i64,
        limit: usize,
    ) -> Result<Vec<AlbumSummary>> {
        self.albums(
            "SELECT album, MAX(id) AS last, COUNT(*) AS n FROM plays
             WHERE album_id IS NOT NULL AND played_at >= ?1
             GROUP BY album_id HAVING n >= ?2 ORDER BY n DESC, last DESC LIMIT ?3",
            params![since, min_plays, limit as i64],
        )
    }

    pub fn clear_plays(&self) -> Result<()> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        db.conn.execute("DELETE FROM plays", [])?;
        Ok(())
    }

    /// `SELECT MAX(id)` désigne la ligne de l'écoute la plus récente de l'album (colonne
    /// « nue » de SQLite) : sa copie de l'album est la plus fraîche. Une copie illisible
    /// (autre version) est ignorée.
    fn albums(&self, sql: &str, args: impl rusqlite::Params) -> Result<Vec<AlbumSummary>> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let mut stmt = db.conn.prepare(sql)?;
        let rows = stmt.query_map(args, |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for json in rows {
            match serde_json::from_str(&json?) {
                Ok(album) => out.push(album),
                Err(e) => tracing::debug!("album de l'historique illisible : {e}"),
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn album(id: u64) -> AlbumSummary {
        AlbumSummary {
            id,
            title: format!("Album {id}"),
            artists: vec![],
            cover_url: None,
            cover_large_url: None,
            color: None,
            release_date: None,
            number_of_tracks: None,
            album_type: None,
            explicit: false,
            ai: false,
            hires: false,
        }
    }

    fn play(store: &Store, track: u64, album_id: Option<u64>, at: i64) {
        let album = album_id.map(album);
        store
            .add_play(&Play {
                track: TrackId(track),
                album: album.as_ref(),
                source: None,
                played_at: at,
            })
            .unwrap();
    }

    fn ids(albums: Vec<AlbumSummary>) -> Vec<u64> {
        albums.into_iter().map(|a| a.id).collect()
    }

    #[test]
    fn recent_albums_are_distinct_and_ordered() {
        let store = Store::in_memory().unwrap();
        play(&store, 1, Some(10), 1);
        play(&store, 2, Some(20), 2);
        play(&store, 3, None, 3);
        play(&store, 4, Some(10), 4);
        play(&store, 5, Some(30), 5);
        assert_eq!(ids(store.recent_albums(10).unwrap()), [30, 10, 20]);
        assert_eq!(ids(store.recent_albums(2).unwrap()), [30, 10]);
    }

    #[test]
    fn top_albums_count_recent_plays() {
        let store = Store::in_memory().unwrap();
        for (track, album, at) in [(1, 10, 100), (2, 10, 100), (3, 10, 100), (4, 20, 5)] {
            play(&store, track, Some(album), at);
        }
        play(&store, 5, Some(20), 200);
        play(&store, 6, Some(30), 200);
        play(&store, 7, Some(30), 200);
        // Album 20 : une seule écoute depuis 50, sous le minimum de 2.
        assert_eq!(ids(store.top_albums(50, 2, 10).unwrap()), [10, 30]);
        assert_eq!(ids(store.top_albums(0, 2, 10).unwrap()), [10, 30, 20]);
    }

    #[test]
    fn prune_and_clear() {
        let store = Store::in_memory().unwrap();
        for i in 0..(MAX_PLAYS as u64 + 3) {
            play(&store, i, Some(i % 7), i as i64);
        }
        let count: i64 = {
            let db = store.db.lock().unwrap();
            db.conn
                .query_row("SELECT COUNT(*) FROM plays", [], |r| r.get(0))
                .unwrap()
        };
        assert_eq!(count, MAX_PLAYS);
        store.clear_plays().unwrap();
        assert!(store.recent_albums(10).unwrap().is_empty());
    }
}
