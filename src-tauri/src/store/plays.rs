//! Local listening history (#202): nothing is sent to TIDAL or anywhere else (the optional
//! sending to ListenBrainz has its own queue, `scrobbles.rs`, #431).
//!
//! One row per counted play (rule of `player::listen`), only the `MAX_PLAYS` most recent.
//! The rows only keep ids, the main artist, the duration and the source (`PlaySource`,
//! #124); the metadata lives once per track and per album in `played_tracks` and
//! `played_albums` (last copy seen, enough to show a card without a request), pruned with
//! the rows (#326). `played_artists` keeps the photo of the artists in the statistics,
//! asked of TIDAL once (`NULL`: artist without a photo). The statistics are in `stats.rs`,
//! the exchange with another device in `sync.rs` (#430).

use rusqlite::{Connection, params};

use super::formats::{self, PlayFormat};
use super::stats::Span;
use super::{Result, Store, StoreError};
use crate::player::source::PlaySource;
use crate::tidal::models::{AlbumSummary, Track};

/// Most plays kept.
pub const MAX_PLAYS: i64 = 50_000;
/// Pruning in batches: orphaned metadata is only looked for once per batch.
const PRUNE_BATCH: i64 = 1_000;

/// Original shape (#202), completed by `migrate`. The `album` column (a JSON copy per
/// row) is no longer filled since 0.5.33.
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

/// Adds the main artist and the duration (#326), and takes the album copies out of the
/// rows: the last copy of each album goes to `played_albums`, the artist is taken from that
/// copy (the track's artists, `player::persist::album_of`). The duration of old plays stays
/// unknown until their track is completed (`save_played_tracks`).
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
    // Source device of a received play (#430); `NULL` = played here.
    let has_device: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('plays') WHERE name = 'device'",
        [],
        |r| r.get(0),
    )?;
    if !has_device {
        conn.execute_batch("ALTER TABLE plays ADD COLUMN device TEXT;")?;
    }
    conn.execute_batch(INDEXES)?;
    Ok(())
}

/// A play to record.
pub struct Play<'a> {
    pub track: &'a Track,
    pub album: &'a AlbumSummary,
    pub source: Option<&'a PlaySource>,
    /// Milliseconds since the Unix epoch.
    pub played_at: i64,
    /// Format actually played (#524); `None` when unknown.
    pub format: Option<&'a PlayFormat>,
}

const UPSERT_TRACK: &str = "INSERT INTO played_tracks (id, json) VALUES (?1, ?2)
                            ON CONFLICT(id) DO UPDATE SET json = excluded.json";
const UPSERT_ALBUM: &str = "INSERT INTO played_albums (id, json) VALUES (?1, ?2)
                            ON CONFLICT(id) DO UPDATE SET json = excluded.json";

impl Store {
    /// Adds a play and prunes beyond `MAX_PLAYS`.
    pub fn add_play(&self, play: &Play) -> Result<()> {
        let track = serde_json::to_string(play.track)?;
        let album = serde_json::to_string(play.album)?;
        let source = play.source.map(serde_json::to_string).transpose()?;
        let artist = play.track.artists.first();
        let (quality, codec, bit_depth, sample_rate, bit_perfect) = formats::values(play.format);
        let mut db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let tx = db.conn.transaction()?;
        tx.execute(
            &format!(
                "INSERT INTO plays
                     (track_id, album_id, source, played_at, artist_id, artist, duration,
                      {})
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                formats::COLUMNS
            ),
            params![
                play.track.id.0 as i64,
                play.album.id as i64,
                source,
                play.played_at,
                artist.map(|a| a.id as i64),
                artist.map(|a| a.name.as_str()),
                play.track.duration,
                quality,
                codec,
                bit_depth,
                sample_rate,
                bit_perfect,
            ],
        )?;
        tx.execute(UPSERT_TRACK, params![play.track.id.0 as i64, track])?;
        tx.execute(UPSERT_ALBUM, params![play.album.id as i64, album])?;
        prune(&tx)?;
        tx.commit()?;
        Ok(())
    }

    /// Completes the tracks of old plays (recorded without their metadata), and their
    /// duration.
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

    /// Played albums, most recent first, without duplicates.
    pub fn recent_albums(&self, limit: usize) -> Result<Vec<AlbumSummary>> {
        Ok(self
            .counted_albums(
                "SELECT a.json, MAX(p.played_at) AS last, COUNT(*) FROM plays p
                 JOIN played_albums a ON a.id = p.album_id
                 GROUP BY p.album_id ORDER BY last DESC, MAX(p.id) DESC LIMIT ?1",
                params![limit as i64],
            )?
            .into_iter()
            .map(|(album, _)| album)
            .collect())
    }

    /// Most played albums over `span`, with at least `min_plays` plays, and their number
    /// of plays.
    pub fn top_albums(
        &self,
        span: Span,
        min_plays: i64,
        limit: usize,
    ) -> Result<Vec<(AlbumSummary, i64)>> {
        self.counted_albums(
            "SELECT a.json, MAX(p.played_at) AS last, COUNT(*) AS n FROM plays p
             JOIN played_albums a ON a.id = p.album_id
             WHERE p.played_at >= ?1 AND p.played_at < ?2
             GROUP BY p.album_id HAVING n >= ?3
             ORDER BY n DESC, last DESC, MAX(p.id) DESC LIMIT ?4",
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

    /// A query whose columns 0 and 2 are the album copy and a number of plays. An
    /// unreadable copy (another version) is skipped.
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
                Err(e) => tracing::debug!("history album unreadable: {e}"),
            }
        }
        Ok(out)
    }
}

/// Keeps the `MAX_PLAYS` most recent plays **by date**, in batches of `PRUNE_BATCH`, then
/// removes the metadata that no play refers to anymore. The date rather than the id: a play
/// received from another device (#430) comes with a recent id.
pub(super) fn prune(conn: &Connection) -> Result<()> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM plays", [], |r| r.get(0))?;
    if count <= MAX_PLAYS + PRUNE_BATCH {
        return Ok(());
    }
    conn.execute(
        "DELETE FROM plays WHERE id IN (
             SELECT id FROM plays ORDER BY played_at DESC, id DESC LIMIT -1 OFFSET ?1)",
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

    /// Play of track `track` on album `album_id`, by artist `album_id + 100`.
    pub fn play(store: &Store, track_id: u64, album_id: u64, at: i64) {
        store
            .add_play(&Play {
                track: &track(track_id, album_id, album_id + 100),
                album: &album(album_id),
                source: None,
                played_at: at,
                format: None,
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
        // Album 20: a single play since 50, under the minimum of 2.
        assert_eq!(top(50), [(10, 3), (30, 2)]);
        assert_eq!(top(0), [(10, 3), (30, 2), (20, 2)]);
    }

    #[test]
    fn prune_and_clear() {
        let store = Store::in_memory().unwrap();
        let total = MAX_PLAYS + PRUNE_BATCH + 1;
        {
            // Direct insert: `add_play` one by one would be slow for 50,000 rows.
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
        // Tracks of the pruned plays removed, the others kept.
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
        // Shape from before 0.5.33: no artist or duration columns.
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
        newer.title = "Album 10 (newer copy)".into();
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
        // A second opening does nothing again.
        migrate(&conn).unwrap();
        let store = Store::init(conn).unwrap();
        let recent = store.recent_albums(10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].title, "Album 10 (newer copy)");
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
