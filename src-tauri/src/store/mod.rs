//! Persistance SQLite : file d'attente, position de lecture, réglages et métadonnées des
//! titres en file ; copies des listes affichées (`snapshots`, #181) ; historique d'écoute
//! (`plays`, #202) et ses statistiques (`stats`, #326).
//!
//! La table `tracks` ne contient que les titres référencés par la file sauvegardée : chaque
//! sauvegarde de la file n'écrit que les titres absents et supprime ceux qui ne sont plus
//! référencés, dans la même transaction (#90). L'ensemble des ids présents est tenu en
//! mémoire sous le même verrou que la connexion, et mis à jour seulement après le commit.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::player::queue::Queue;
use crate::player::volume::Volume;
use crate::tidal::models::{Track, TrackId};

mod plays;
mod snapshots;
mod stats;

pub use plays::Play;
pub use snapshots::SnapshotLimit;

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
    CREATE TABLE IF NOT EXISTS snapshots (
        key     TEXT PRIMARY KEY,
        used_at INTEGER NOT NULL,
        size    INTEGER NOT NULL,
        json    TEXT NOT NULL
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

struct Db {
    conn: Connection,
    /// Ids présents dans la table `tracks`.
    saved: HashSet<u64>,
}

pub struct Store {
    db: Mutex<Db>,
    /// Valeurs JSON de `kv` déjà lues (`None` : absente de la base). Verrou distinct de la
    /// connexion : relire un réglage n'attend jamais une sauvegarde de la file. N'est
    /// modifié que sous le verrou de `db`, pour rester dans l'ordre des écritures.
    settings: Mutex<HashMap<String, Option<String>>>,
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
        conn.execute_batch(plays::SCHEMA)?;
        plays::migrate(&conn)?;
        let saved = conn
            .prepare("SELECT id FROM tracks")?
            .query_map([], |r| r.get::<_, i64>(0))?
            .map(|id| id.map(|id| id as u64))
            .collect::<rusqlite::Result<_>>()?;
        Ok(Self {
            db: Mutex::new(Db { conn, saved }),
            settings: Mutex::default(),
        })
    }

    /// Sauvegarde la position, et la file si elle est fournie. `lookup` donne les
    /// métadonnées des titres à ajouter à la table : il n'est appelé que pour les titres
    /// référencés qui n'y sont pas encore (un titre inconnu sera retenté à la sauvegarde
    /// suivante). Les titres qui ne sont plus référencés sont supprimés.
    pub fn save_player(
        &self,
        queue: Option<&Queue>,
        position: f64,
        lookup: impl FnOnce(&[TrackId]) -> Vec<Track>,
    ) -> Result<()> {
        let mut db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let Db { conn, saved } = &mut *db;
        let tx = conn.transaction()?;
        let upsert = "INSERT INTO kv (key, value) VALUES (?1, ?2)
                      ON CONFLICT(key) DO UPDATE SET value = excluded.value";
        tx.execute(upsert, params![KEY_POSITION, position.to_string()])?;
        let mut added = Vec::new();
        let mut stale = Vec::new();
        if let Some(queue) = queue {
            tx.execute(upsert, params![KEY_QUEUE, serde_json::to_string(queue)?])?;
            let referenced = queue.referenced_ids();
            let missing: Vec<TrackId> = referenced
                .iter()
                .copied()
                .filter(|id| !saved.contains(&id.0))
                .collect();
            let tracks = if missing.is_empty() {
                Vec::new()
            } else {
                lookup(&missing)
            };
            let mut insert = tx.prepare(
                "INSERT INTO tracks (id, json) VALUES (?1, ?2)
                 ON CONFLICT(id) DO UPDATE SET json = excluded.json",
            )?;
            for t in &tracks {
                insert.execute(params![t.id.0 as i64, serde_json::to_string(t)?])?;
                added.push(t.id.0);
            }
            let referenced: HashSet<u64> = referenced.iter().map(|id| id.0).collect();
            stale.extend(saved.iter().copied().filter(|id| !referenced.contains(id)));
            let mut delete = tx.prepare("DELETE FROM tracks WHERE id = ?1")?;
            for id in &stale {
                delete.execute([*id as i64])?;
            }
        }
        tx.commit()?;
        for id in stale {
            saved.remove(&id);
        }
        saved.extend(added);
        if let Ok(mut settings) = self.settings.lock() {
            settings.remove(KEY_POSITION);
            settings.remove(KEY_QUEUE);
        }
        Ok(())
    }

    /// Valeur JSON d'un réglage (`None` s'il n'a jamais été enregistré). Lue une fois en
    /// base, puis en mémoire.
    pub fn get_setting<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let cached = self
            .settings
            .lock()
            .map_err(|_| StoreError::Poisoned)?
            .get(key)
            .cloned();
        let json = match cached {
            Some(json) => json,
            None => {
                let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
                let json: Option<String> = db
                    .conn
                    .query_row("SELECT value FROM kv WHERE key = ?1", [key], |r| r.get(0))
                    .optional()?;
                self.settings
                    .lock()
                    .map_err(|_| StoreError::Poisoned)?
                    .insert(key.to_owned(), json.clone());
                json
            }
        };
        Ok(json.map(|j| serde_json::from_str(&j)).transpose()?)
    }

    pub fn set_setting<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string(value)?;
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        db.conn.execute(
            "INSERT INTO kv (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, json],
        )?;
        self.settings
            .lock()
            .map_err(|_| StoreError::Poisoned)?
            .insert(key.to_owned(), Some(json));
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
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let conn = &db.conn;
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
mod tests;
