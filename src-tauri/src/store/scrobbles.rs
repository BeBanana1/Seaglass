//! Queue of plays to send to ListenBrainz (#431): each play counted here, ready to go
//! (the API's JSON, `listened_at` included), waiting for the network or for the service to
//! be available. Only plays made on this device go in: those received from another device
//! (#430) go through `plays` without ever being queued.

use rusqlite::params;

use super::{Result, Store, StoreError};

/// Most waiting plays: beyond that, the oldest are dropped.
pub const MAX_SCROBBLES: i64 = 10_000;

pub(super) const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS scrobbles (
        id     INTEGER PRIMARY KEY AUTOINCREMENT,
        listen TEXT NOT NULL
    );
";

/// A waiting play: its id in the queue and its JSON.
#[derive(Debug, Clone, PartialEq)]
pub struct Scrobble {
    pub id: i64,
    pub listen: String,
}

impl Store {
    /// Queues a play, dropping the oldest beyond `MAX_SCROBBLES`.
    pub fn queue_scrobble(&self, listen: &str) -> Result<()> {
        let mut db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let tx = db.conn.transaction()?;
        tx.execute("INSERT INTO scrobbles (listen) VALUES (?1)", [listen])?;
        tx.execute(
            "DELETE FROM scrobbles WHERE id <= (SELECT MAX(id) FROM scrobbles) - ?1",
            [MAX_SCROBBLES],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The `limit` oldest plays.
    pub fn pending_scrobbles(&self, limit: usize) -> Result<Vec<Scrobble>> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let mut stmt = db
            .conn
            .prepare("SELECT id, listen FROM scrobbles ORDER BY id LIMIT ?1")?;
        let rows = stmt.query_map([limit as i64], |r| {
            Ok(Scrobble {
                id: r.get(0)?,
                listen: r.get(1)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Removes plays that were sent (or refused for good).
    pub fn drop_scrobbles(&self, ids: &[i64]) -> Result<()> {
        let mut db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let tx = db.conn.transaction()?;
        {
            let mut delete = tx.prepare("DELETE FROM scrobbles WHERE id = ?1")?;
            for id in ids {
                delete.execute(params![id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Empties the queue (account removed).
    pub fn clear_scrobbles(&self) -> Result<()> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        db.conn.execute("DELETE FROM scrobbles", [])?;
        Ok(())
    }

    pub fn scrobble_count(&self) -> Result<u32> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        Ok(db
            .conn
            .query_row("SELECT COUNT(*) FROM scrobbles", [], |r| r.get(0))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_send_and_clear() {
        let store = Store::in_memory().unwrap();
        for i in 0..3 {
            store.queue_scrobble(&format!("{{\"n\":{i}}}")).unwrap();
        }
        assert_eq!(store.scrobble_count().unwrap(), 3);
        let first = store.pending_scrobbles(2).unwrap();
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].listen, "{\"n\":0}");
        store
            .drop_scrobbles(&first.iter().map(|s| s.id).collect::<Vec<_>>())
            .unwrap();
        let rest = store.pending_scrobbles(10).unwrap();
        assert_eq!(rest.len(), 1);
        assert_eq!(rest[0].listen, "{\"n\":2}");
        store.clear_scrobbles().unwrap();
        assert_eq!(store.scrobble_count().unwrap(), 0);
    }

    #[test]
    fn oldest_are_dropped_past_the_cap() {
        let store = Store::in_memory().unwrap();
        for i in 0..MAX_SCROBBLES + 5 {
            store.queue_scrobble(&i.to_string()).unwrap();
        }
        assert_eq!(store.scrobble_count().unwrap(), MAX_SCROBBLES as u32);
        assert_eq!(store.pending_scrobbles(1).unwrap()[0].listen, "5");
    }
}
