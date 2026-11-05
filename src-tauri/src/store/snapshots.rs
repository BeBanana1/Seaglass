//! Dernières copies des listes affichées (#181) : pages de playlist et onglets de la
//! bibliothèque, gardées d'un lancement à l'autre pour être affichées aussitôt, puis
//! revalidées ou relues par l'appelant.
//!
//! Une ligne par clé (`playlist:<uuid>`, `library:<onglet>`), en JSON. `used_at` est une
//! horloge logique (le plus grand + 1), mise à jour à l'écriture comme à la lecture : elle
//! ordonne les copies pour l'élagage, sans dépendre de l'heure du système.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::{Result, Store, StoreError};

/// Limites d'un groupe de copies de même préfixe.
#[derive(Debug, Clone, Copy)]
pub struct SnapshotLimit {
    pub prefix: &'static str,
    /// Copies gardées au plus.
    pub count: usize,
    /// Somme des tailles (`size`, en titres) gardée au plus ; la plus récente est toujours
    /// gardée, même si elle dépasse seule la limite.
    pub size: usize,
}

const NEXT_CLOCK: &str = "(SELECT COALESCE(MAX(used_at), 0) + 1 FROM snapshots)";

impl Store {
    /// Copie gardée sous `key`. Une copie illisible (format d'une autre version) est
    /// effacée et vaut `None`.
    pub fn get_snapshot<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let json: Option<String> = {
            let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
            let json = db
                .conn
                .query_row("SELECT json FROM snapshots WHERE key = ?1", [key], |r| {
                    r.get(0)
                })
                .optional()?;
            if json.is_some() {
                db.conn.execute(
                    &format!("UPDATE snapshots SET used_at = {NEXT_CLOCK} WHERE key = ?1"),
                    [key],
                )?;
            }
            json
        };
        let Some(json) = json else {
            return Ok(None);
        };
        match serde_json::from_str(&json) {
            Ok(value) => Ok(Some(value)),
            Err(e) => {
                tracing::warn!("copie {key} illisible, effacée : {e}");
                self.remove_snapshot(key)?;
                Ok(None)
            }
        }
    }

    /// Enregistre la copie (`size` : nombre de titres, pour `limit`), puis élague le groupe
    /// de `limit` s'il est donné.
    pub fn put_snapshot<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        size: usize,
        limit: Option<SnapshotLimit>,
    ) -> Result<()> {
        // Sérialisée avant de prendre le verrou : une longue liste fait quelques Mo.
        let json = serde_json::to_string(value)?;
        let mut db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let tx = db.conn.transaction()?;
        tx.execute(
            &format!(
                "INSERT INTO snapshots (key, used_at, size, json) VALUES (?1, {NEXT_CLOCK}, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET used_at = excluded.used_at,
                     size = excluded.size, json = excluded.json"
            ),
            params![key, size as i64, json],
        )?;
        if let Some(limit) = limit {
            let rows: Vec<(String, i64)> = tx
                .prepare(
                    "SELECT key, size FROM snapshots WHERE substr(key, 1, ?1) = ?2
                     ORDER BY used_at DESC",
                )?
                .query_map(params![limit.prefix.len() as i64, limit.prefix], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })?
                .collect::<rusqlite::Result<_>>()?;
            let sizes: Vec<usize> = rows.iter().map(|(_, s)| (*s).max(0) as usize).collect();
            let keep = kept(&sizes, limit);
            let mut delete = tx.prepare("DELETE FROM snapshots WHERE key = ?1")?;
            for (key, _) in &rows[keep..] {
                delete.execute([key])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn remove_snapshot(&self, key: &str) -> Result<()> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        db.conn
            .execute("DELETE FROM snapshots WHERE key = ?1", [key])?;
        Ok(())
    }

    /// Changement de compte : les copies décrivent la bibliothèque de l'ancien.
    pub fn clear_snapshots(&self) -> Result<()> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        db.conn.execute("DELETE FROM snapshots", [])?;
        Ok(())
    }
}

/// Nombre de copies gardées, en partant des plus récentes (`sizes` dans cet ordre).
fn kept(sizes: &[usize], limit: SnapshotLimit) -> usize {
    let mut total = 0;
    sizes
        .iter()
        .take(limit.count)
        .position(|size| {
            total += size;
            total > limit.size
        })
        .map_or(sizes.len().min(limit.count), |i| i.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMIT: SnapshotLimit = SnapshotLimit {
        prefix: "p:",
        count: 3,
        size: 10,
    };

    fn keys(store: &Store) -> Vec<String> {
        let db = store.db.lock().unwrap();
        let mut stmt = db
            .conn
            .prepare("SELECT key FROM snapshots ORDER BY used_at DESC")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    fn round_trip_and_remove() {
        let store = Store::in_memory().unwrap();
        assert_eq!(store.get_snapshot::<Vec<u32>>("a").unwrap(), None);
        store.put_snapshot("a", &vec![1u32, 2], 2, None).unwrap();
        store.put_snapshot("a", &vec![3u32], 1, None).unwrap();
        assert_eq!(store.get_snapshot::<Vec<u32>>("a").unwrap(), Some(vec![3]));
        store.remove_snapshot("a").unwrap();
        assert_eq!(store.get_snapshot::<Vec<u32>>("a").unwrap(), None);
    }

    #[test]
    fn unreadable_copy_is_dropped() {
        let store = Store::in_memory().unwrap();
        store.put_snapshot("a", &"text", 1, None).unwrap();
        assert_eq!(store.get_snapshot::<Vec<u32>>("a").unwrap(), None);
        assert!(keys(&store).is_empty());
    }

    #[test]
    fn pruned_by_count_keeping_recently_read() {
        let store = Store::in_memory().unwrap();
        for k in ["p:1", "p:2", "p:3"] {
            store.put_snapshot(k, &0, 1, Some(LIMIT)).unwrap();
        }
        // Lue, la plus ancienne repasse devant.
        store.get_snapshot::<i32>("p:1").unwrap();
        store.put_snapshot("other", &0, 100, None).unwrap();
        store.put_snapshot("p:4", &0, 1, Some(LIMIT)).unwrap();
        assert_eq!(keys(&store), ["p:4", "other", "p:1", "p:3"]);
    }

    #[test]
    fn pruned_by_size_keeping_the_latest() {
        let store = Store::in_memory().unwrap();
        store.put_snapshot("p:a", &0, 5, Some(LIMIT)).unwrap();
        store.put_snapshot("p:b", &0, 5, Some(LIMIT)).unwrap();
        store.put_snapshot("p:c", &0, 1, Some(LIMIT)).unwrap();
        assert_eq!(keys(&store), ["p:c", "p:b"]);
        store.put_snapshot("p:huge", &0, 50, Some(LIMIT)).unwrap();
        assert_eq!(keys(&store), ["p:huge"]);
    }

    #[test]
    fn clear_forgets_everything() {
        let store = Store::in_memory().unwrap();
        store.put_snapshot("p:a", &0, 1, Some(LIMIT)).unwrap();
        store.put_snapshot("library:tracks", &0, 1, None).unwrap();
        store.clear_snapshots().unwrap();
        assert!(keys(&store).is_empty());
    }
}
