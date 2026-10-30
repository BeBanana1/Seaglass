//! Persistance SQLite : file d'attente, position de lecture et métadonnées des titres en file.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::player::queue::Queue;
use crate::player::volume::Volume;
use crate::tidal::models::Track;

const SCHEMA: &str = "
    PRAGMA journal_mode = WAL;
    PRAGMA synchronous = NORMAL;
    CREATE TABLE IF NOT EXISTS kv (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS tracks (
        id   INTEGER PRIMARY KEY,
        json TEXT NOT NULL
    );
";

const KEY_QUEUE: &str = "player.queue";
const KEY_POSITION: &str = "player.position";
const KEY_VOLUME: &str = "player.volume";

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("SQLite : {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("JSON : {0}")]
    Json(#[from] serde_json::Error),
    #[error(
        "{}",
        crate::i18n::tr("verrou de la base empoisonné", "database lock poisoned")
    )]
    Poisoned,
}

pub type Result<T> = std::result::Result<T, StoreError>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedPlayer {
    pub queue: Queue,
    pub position: f64,
    pub tracks: Vec<Track>,
}

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        Self::init(Connection::open(path)?)
    }

    #[cfg(test)]
    fn in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Sauvegarde la position ; la file et les titres seulement si `queue` est fourni.
    pub fn save_player(&self, queue: Option<(&Queue, &[Track])>, position: f64) -> Result<()> {
        let mut conn = self.conn.lock().map_err(|_| StoreError::Poisoned)?;
        let tx = conn.transaction()?;
        let upsert = "INSERT INTO kv (key, value) VALUES (?1, ?2)
                      ON CONFLICT(key) DO UPDATE SET value = excluded.value";
        tx.execute(upsert, params![KEY_POSITION, position.to_string()])?;
        if let Some((queue, tracks)) = queue {
            tx.execute(upsert, params![KEY_QUEUE, serde_json::to_string(queue)?])?;
            let mut stmt = tx.prepare(
                "INSERT INTO tracks (id, json) VALUES (?1, ?2)
                 ON CONFLICT(id) DO UPDATE SET json = excluded.json",
            )?;
            for t in tracks {
                stmt.execute(params![t.id.0 as i64, serde_json::to_string(t)?])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Valeur JSON d'un réglage (`None` s'il n'a jamais été enregistré).
    pub fn get_setting<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let conn = self.conn.lock().map_err(|_| StoreError::Poisoned)?;
        let json: Option<String> = conn
            .query_row("SELECT value FROM kv WHERE key = ?1", [key], |r| r.get(0))
            .optional()?;
        Ok(json.map(|j| serde_json::from_str(&j)).transpose()?)
    }

    pub fn set_setting<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| StoreError::Poisoned)?;
        conn.execute(
            "INSERT INTO kv (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, serde_json::to_string(value)?],
        )?;
        Ok(())
    }

    /// Volume sauvegardé, ramené dans les limites (`None` s'il n'a jamais été réglé).
    pub fn load_volume(&self) -> Result<Option<Volume>> {
        Ok(self
            .get_setting::<Volume>(KEY_VOLUME)?
            .map(Volume::sanitized))
    }

    pub fn save_volume(&self, volume: Volume) -> Result<()> {
        self.set_setting(KEY_VOLUME, &volume)
    }

    pub fn load_player(&self) -> Result<Option<SavedPlayer>> {
        let conn = self.conn.lock().map_err(|_| StoreError::Poisoned)?;
        let get = |key: &str| -> Result<Option<String>> {
            Ok(conn
                .query_row("SELECT value FROM kv WHERE key = ?1", [key], |r| r.get(0))
                .optional()?)
        };
        let Some(queue_json) = get(KEY_QUEUE)? else {
            return Ok(None);
        };
        let queue: Queue = serde_json::from_str(&queue_json)?;
        let position = get(KEY_POSITION)?
            .and_then(|p| p.parse().ok())
            .unwrap_or(0.0);

        let mut stmt = conn.prepare("SELECT json FROM tracks WHERE id = ?1")?;
        let mut tracks = Vec::new();
        for id in queue.referenced_ids() {
            let json: Option<String> = stmt.query_row([id.0 as i64], |r| r.get(0)).optional()?;
            if let Some(json) = json {
                tracks.push(serde_json::from_str(&json)?);
            }
        }
        Ok(Some(SavedPlayer {
            queue,
            position,
            tracks,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::{AlbumRef, TrackId};
    use rand::SeedableRng;

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
                color: None,
            },
            explicit: false,
            hires: true,
            isrc: None,
        }
    }

    #[test]
    fn settings_roundtrip() {
        let store = Store::in_memory().unwrap();
        assert_eq!(store.get_setting::<String>("ui").unwrap(), None);
        store.set_setting("ui", &"studio").unwrap();
        store.set_setting("ui", &"atelier").unwrap();
        assert_eq!(
            store.get_setting::<String>("ui").unwrap().as_deref(),
            Some("atelier")
        );
    }

    #[test]
    fn empty_store_has_nothing() {
        let store = Store::in_memory().unwrap();
        assert_eq!(store.load_player().unwrap(), None);
        assert_eq!(store.load_volume().unwrap(), None);
    }

    #[test]
    fn volume_roundtrip_and_clamp() {
        let store = Store::in_memory().unwrap();
        let v = Volume {
            level: 35,
            muted: true,
        };
        store.save_volume(v).unwrap();
        assert_eq!(store.load_volume().unwrap(), Some(v));

        // Valeur hors limites écrite à la main dans la base : ramenée à 100.
        store
            .set_setting(
                KEY_VOLUME,
                &serde_json::json!({ "level": 250, "muted": false }),
            )
            .unwrap();
        assert_eq!(store.load_volume().unwrap(), Some(Volume::default()));
    }

    #[test]
    fn roundtrip_queue_position_and_tracks() {
        let store = Store::in_memory().unwrap();
        let mut q = Queue::default();
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        q.set_context(vec![TrackId(1), TrackId(2)], 0, &mut rng);
        let tracks = vec![track(1), track(2)];

        store.save_player(Some((&q, &tracks)), 12.5).unwrap();
        store.save_player(None, 42.0).unwrap();

        let saved = store.load_player().unwrap().unwrap();
        assert_eq!(saved.queue, q);
        assert_eq!(saved.position, 42.0);
        assert_eq!(saved.tracks, tracks);
    }
}
