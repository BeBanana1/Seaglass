//! Historique d'écoute local (#202) : rien n'est envoyé à TIDAL ni ailleurs.
//!
//! Une ligne par écoute comptée (règle de `player::listen`), les `MAX_PLAYS` plus récentes
//! seulement. Les lignes ne gardent que des identifiants, l'artiste principal, la durée et
//! la source (`PlaySource`, #124) ; les métadonnées vivent une fois par titre et par album
//! dans `played_tracks` et `played_albums` (dernière copie vue, de quoi afficher une carte
//! sans requête), élaguées avec les lignes (#326). `played_artists` garde la photo des
//! artistes des statistiques, demandée une fois à TIDAL (`NULL` : artiste sans photo). Les
//! statistiques sont dans `stats.rs`.

use rusqlite::{Connection, params};

use super::stats::Span;
use super::{Result, Store, StoreError};
use crate::player::source::PlaySource;
use crate::tidal::models::{AlbumSummary, Track};

/// Écoutes gardées au plus.
pub const MAX_PLAYS: i64 = 50_000;
/// Élagage par lots : les métadonnées orphelines ne sont cherchées qu'une fois par lot.
const PRUNE_BATCH: i64 = 1_000;

/// Forme d'origine (#202), complétée par `migrate`. La colonne `album` (copie JSON par
/// ligne) n'est plus remplie depuis la 0.5.33.
pub(super) const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS plays (
        id        INTEGER PRIMARY KEY AUTOINCREMENT,
        track_id  INTEGER NOT NULL,
        album_id  INTEGER,
        album     TEXT,
        source    TEXT,
        played_at INTEGER NOT NULL
    );
    CREATE TABLE IF NOT EXISTS played_tracks (
        id   INTEGER PRIMARY KEY,
        json TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS played_albums (
        id   INTEGER PRIMARY KEY,
        json TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS played_artists (
        id      INTEGER PRIMARY KEY,
        picture TEXT
    );
";

const INDEXES: &str = "
    CREATE INDEX IF NOT EXISTS plays_album ON plays (album_id, id);
    CREATE INDEX IF NOT EXISTS plays_track ON plays (track_id);
    CREATE INDEX IF NOT EXISTS plays_artist ON plays (artist_id);
    CREATE INDEX IF NOT EXISTS plays_at ON plays (played_at);
";

/// Ajoute l'artiste principal et la durée (#326), et sort les copies d'album des lignes :
/// la dernière copie de chaque album part dans `played_albums`, l'artiste est repris de
/// cette copie (les artistes du titre, `player::persist::album_of`). La durée des anciennes
/// écoutes reste inconnue jusqu'à ce que leur titre soit complété (`save_played_tracks`).
pub(super) fn migrate(conn: &Connection) -> Result<()> {
    let migrated: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('plays') WHERE name = 'duration'",
        [],
        |r| r.get(0),
    )?;
    if !migrated {
        conn.execute_batch(
            "BEGIN;
             ALTER TABLE plays ADD COLUMN artist_id INTEGER;
             ALTER TABLE plays ADD COLUMN artist TEXT;
             ALTER TABLE plays ADD COLUMN duration INTEGER;
             INSERT OR REPLACE INTO played_albums (id, json)
                 SELECT album_id, album FROM plays
                 WHERE album_id IS NOT NULL AND album IS NOT NULL ORDER BY id;
             UPDATE plays SET
                 artist_id = json_extract(album, '$.artists[0].id'),
                 artist = json_extract(album, '$.artists[0].name'),
                 album = NULL
                 WHERE album IS NOT NULL AND json_valid(album);
             UPDATE plays SET album = NULL;
             COMMIT;",
        )?;
    }
    conn.execute_batch(INDEXES)?;
    Ok(())
}

/// Une écoute à enregistrer.
pub struct Play<'a> {
    pub track: &'a Track,
    pub album: &'a AlbumSummary,
    pub source: Option<&'a PlaySource>,
    /// Millisecondes depuis l'epoch Unix.
    pub played_at: i64,
}

const UPSERT_TRACK: &str = "INSERT INTO played_tracks (id, json) VALUES (?1, ?2)
                            ON CONFLICT(id) DO UPDATE SET json = excluded.json";
const UPSERT_ALBUM: &str = "INSERT INTO played_albums (id, json) VALUES (?1, ?2)
                            ON CONFLICT(id) DO UPDATE SET json = excluded.json";

impl Store {
    /// Ajoute une écoute et élague au-delà de `MAX_PLAYS`.
    pub fn add_play(&self, play: &Play) -> Result<()> {
        let track = serde_json::to_string(play.track)?;
        let album = serde_json::to_string(play.album)?;
        let source = play.source.map(serde_json::to_string).transpose()?;
        let artist = play.track.artists.first();
        let mut db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let tx = db.conn.transaction()?;
        tx.execute(
            "INSERT INTO plays
                 (track_id, album_id, source, played_at, artist_id, artist, duration)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                play.track.id.0 as i64,
                play.album.id as i64,
                source,
                play.played_at,
                artist.map(|a| a.id as i64),
                artist.map(|a| a.name.as_str()),
                play.track.duration,
            ],
        )?;
        tx.execute(UPSERT_TRACK, params![play.track.id.0 as i64, track])?;
        tx.execute(UPSERT_ALBUM, params![play.album.id as i64, album])?;
        prune(&tx)?;
        tx.commit()?;
        Ok(())
    }

    /// Complète les titres d'anciennes écoutes (enregistrées sans leurs métadonnées), et
    /// leur durée.
    pub fn save_played_tracks(&self, tracks: &[Track]) -> Result<()> {
        let mut db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let tx = db.conn.transaction()?;
        for track in tracks {
            let id = track.id.0 as i64;
            tx.execute(UPSERT_TRACK, params![id, serde_json::to_string(track)?])?;
            tx.execute(
                "UPDATE plays SET duration = ?2 WHERE track_id = ?1 AND duration IS NULL",
                params![id, track.duration],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Albums écoutés, du plus récent au plus ancien, sans doublon.
    pub fn recent_albums(&self, limit: usize) -> Result<Vec<AlbumSummary>> {
        Ok(self
            .counted_albums(
                "SELECT a.json, MAX(p.id) AS last, COUNT(*) FROM plays p
                 JOIN played_albums a ON a.id = p.album_id
                 GROUP BY p.album_id ORDER BY last DESC LIMIT ?1",
                params![limit as i64],
            )?
            .into_iter()
            .map(|(album, _)| album)
            .collect())
    }

    /// Albums les plus écoutés sur `span`, avec au moins `min_plays` écoutes, et
    /// leur nombre d'écoutes.
    pub fn top_albums(
        &self,
        span: Span,
        min_plays: i64,
        limit: usize,
    ) -> Result<Vec<(AlbumSummary, i64)>> {
        self.counted_albums(
            "SELECT a.json, MAX(p.id) AS last, COUNT(*) AS n FROM plays p
             JOIN played_albums a ON a.id = p.album_id
             WHERE p.played_at >= ?1 AND p.played_at < ?2
             GROUP BY p.album_id HAVING n >= ?3 ORDER BY n DESC, last DESC LIMIT ?4",
            params![span.since, span.until, min_plays, limit as i64],
        )
    }

    pub fn clear_plays(&self) -> Result<()> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        db.conn.execute_batch(
            "DELETE FROM plays; DELETE FROM played_tracks; DELETE FROM played_albums;
             DELETE FROM played_artists;",
        )?;
        Ok(())
    }

    /// Requête dont les colonnes 0 et 2 sont la copie de l'album et un nombre d'écoutes.
    /// Une copie illisible (autre version) est ignorée.
    fn counted_albums(
        &self,
        sql: &str,
        args: impl rusqlite::Params,
    ) -> Result<Vec<(AlbumSummary, i64)>> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let mut stmt = db.conn.prepare(sql)?;
        let rows = stmt.query_map(args, |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(2)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (json, count) = row?;
            match serde_json::from_str(&json) {
                Ok(album) => out.push((album, count)),
                Err(e) => tracing::debug!("album de l'historique illisible : {e}"),
            }
        }
        Ok(out)
    }
}

/// Garde les `MAX_PLAYS` écoutes les plus récentes, par lots de `PRUNE_BATCH`, puis retire
/// les métadonnées que plus aucune écoute ne référence. Les ids se suivent (AUTOINCREMENT,
/// seul le début est supprimé) : l'écart entre le premier et le dernier suffit.
fn prune(conn: &Connection) -> Result<()> {
    let span: Option<i64> =
        conn.query_row("SELECT MAX(id) - MIN(id) + 1 FROM plays", [], |r| r.get(0))?;
    if span.unwrap_or(0) <= MAX_PLAYS + PRUNE_BATCH {
        return Ok(());
    }
    conn.execute(
        "DELETE FROM plays WHERE id <= (SELECT MAX(id) FROM plays) - ?1",
        [MAX_PLAYS],
    )?;
    conn.execute_batch(
        "DELETE FROM played_tracks WHERE id NOT IN (SELECT track_id FROM plays);
         DELETE FROM played_albums WHERE id NOT IN
             (SELECT album_id FROM plays WHERE album_id IS NOT NULL);
         DELETE FROM played_artists WHERE id NOT IN
             (SELECT artist_id FROM plays WHERE artist_id IS NOT NULL);",
    )?;
    Ok(())
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::tidal::models::{AlbumRef, ArtistRef, TrackId};

    pub fn track(id: u64, album_id: u64, artist: u64) -> Track {
        Track {
            id: TrackId(id),
            title: format!("Titre {id}"),
            version: None,
            duration: 200,
            artists: vec![ArtistRef {
                id: artist,
                name: format!("Artiste {artist}"),
            }],
            album: AlbumRef {
                id: album_id,
                title: format!("Album {album_id}"),
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

    pub fn album(id: u64) -> AlbumSummary {
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

    /// Écoute du titre `track` sur l'album `album_id`, par l'artiste `album_id + 100`.
    pub fn play(store: &Store, track_id: u64, album_id: u64, at: i64) {
        store
            .add_play(&Play {
                track: &track(track_id, album_id, album_id + 100),
                album: &album(album_id),
                source: None,
                played_at: at,
            })
            .unwrap();
    }

    fn ids(albums: Vec<AlbumSummary>) -> Vec<u64> {
        albums.into_iter().map(|a| a.id).collect()
    }

    fn count(store: &Store, table: &str) -> i64 {
        let db = store.db.lock().unwrap();
        db.conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn recent_albums_are_distinct_and_ordered() {
        let store = Store::in_memory().unwrap();
        play(&store, 1, 10, 1);
        play(&store, 2, 20, 2);
        play(&store, 4, 10, 4);
        play(&store, 5, 30, 5);
        assert_eq!(ids(store.recent_albums(10).unwrap()), [30, 10, 20]);
        assert_eq!(ids(store.recent_albums(2).unwrap()), [30, 10]);
    }

    #[test]
    fn top_albums_count_recent_plays() {
        let store = Store::in_memory().unwrap();
        for (track, album, at) in [(1, 10, 100), (2, 10, 100), (3, 10, 100), (4, 20, 5)] {
            play(&store, track, album, at);
        }
        play(&store, 5, 20, 200);
        play(&store, 6, 30, 200);
        play(&store, 7, 30, 200);
        let top = |since| {
            store
                .top_albums(Span::since(since), 2, 10)
                .unwrap()
                .into_iter()
                .map(|(a, n)| (a.id, n))
                .collect::<Vec<_>>()
        };
        // Album 20 : une seule écoute depuis 50, sous le minimum de 2.
        assert_eq!(top(50), [(10, 3), (30, 2)]);
        assert_eq!(top(0), [(10, 3), (30, 2), (20, 2)]);
    }

    #[test]
    fn prune_and_clear() {
        let store = Store::in_memory().unwrap();
        let total = MAX_PLAYS + PRUNE_BATCH + 1;
        {
            // Insertion directe : `add_play` un par un serait lent pour 50 000 lignes.
            let mut db = store.db.lock().unwrap();
            let tx = db.conn.transaction().unwrap();
            for i in 0..total - 1 {
                tx.execute(
                    "INSERT INTO plays (track_id, album_id, played_at) VALUES (?1, 1, ?1)",
                    [i],
                )
                .unwrap();
                tx.execute(UPSERT_TRACK, params![i, "{}"]).unwrap();
            }
            tx.commit().unwrap();
        }
        play(&store, 999_999, 1, total);
        assert_eq!(count(&store, "plays"), MAX_PLAYS);
        // Titres des écoutes élaguées retirés, les autres gardés.
        assert_eq!(count(&store, "played_tracks"), MAX_PLAYS);
        store.clear_plays().unwrap();
        assert!(store.recent_albums(10).unwrap().is_empty());
        assert_eq!(count(&store, "played_tracks"), 0);
        assert_eq!(count(&store, "played_albums"), 0);
        assert_eq!(count(&store, "played_artists"), 0);
    }

    #[test]
    fn migrates_old_rows() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        // Forme d'avant la 0.5.33 : pas de colonnes d'artiste ni de durée.
        conn.execute_batch(
            "DROP TABLE plays;
             CREATE TABLE plays (
                 id INTEGER PRIMARY KEY AUTOINCREMENT, track_id INTEGER NOT NULL,
                 album_id INTEGER, album TEXT, source TEXT, played_at INTEGER NOT NULL);",
        )
        .unwrap();
        let mut old = album(10);
        old.artists = vec![ArtistRef {
            id: 7,
            name: "Ancien".into(),
        }];
        let mut newer = old.clone();
        newer.title = "Album 10 (copie récente)".into();
        for (track, copy) in [(1, &old), (2, &newer)] {
            conn.execute(
                "INSERT INTO plays (track_id, album_id, album, played_at) VALUES (?1, 10, ?2, 1)",
                params![track, serde_json::to_string(copy).unwrap()],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO plays (track_id, album_id, album, played_at) VALUES (3, NULL, NULL, 1)",
            [],
        )
        .unwrap();
        migrate(&conn).unwrap();
        // Une seconde ouverture ne refait rien.
        migrate(&conn).unwrap();
        let store = Store::init(conn).unwrap();
        let recent = store.recent_albums(10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].title, "Album 10 (copie récente)");
        let db = store.db.lock().unwrap();
        let artists: Vec<(Option<i64>, Option<String>, Option<String>)> = db
            .conn
            .prepare("SELECT artist_id, artist, album FROM plays ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            artists,
            [
                (Some(7), Some("Ancien".into()), None),
                (Some(7), Some("Ancien".into()), None),
                (None, None, None),
            ]
        );
    }
}
