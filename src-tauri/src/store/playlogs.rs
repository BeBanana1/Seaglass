//! Send queue of TIDAL's play log (#34): each listen of this device, its event body ready
//! (event uuid and time fixed, so that a resend is the same event), until the collector
//! takes it. No token here: it is added at send time. Each listen keeps the TIDAL user it
//! was heard under, so that it never goes out under another account's token.

use rusqlite::params;

use super::{Result, Store, StoreError};

/// Listens waiting at most: beyond, the oldest are dropped.
pub const MAX_PLAY_LOGS: i64 = 10_000;

pub(super) const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS play_logs (
        id      INTEGER PRIMARY KEY AUTOINCREMENT,
        uuid    TEXT NOT NULL,
        user_id INTEGER NOT NULL,
        body    TEXT NOT NULL
    );
";

/// A queued listen.
#[derive(Debug, Clone, PartialEq)]
pub struct QueuedPlayLog {
    pub id: i64,
    pub uuid: String,
    pub user_id: u64,
    pub body: String,
}

impl Store {
    pub fn queue_play_log(&self, uuid: &str, user_id: u64, body: &str) -> Result<()> {
        let mut db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let tx = db.conn.transaction()?;
        tx.execute(
            "INSERT INTO play_logs (uuid, user_id, body) VALUES (?1, ?2, ?3)",
            params![uuid, user_id as i64, body],
        )?;
        tx.execute(
            "DELETE FROM play_logs WHERE id <= (SELECT MAX(id) FROM play_logs) - ?1",
            [MAX_PLAY_LOGS],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The `limit` oldest listens.
    pub fn pending_play_logs(&self, limit: usize) -> Result<Vec<QueuedPlayLog>> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let mut stmt = db
            .conn
            .prepare("SELECT id, uuid, user_id, body FROM play_logs ORDER BY id LIMIT ?1")?;
        let rows = stmt.query_map([limit as i64], |r| {
            Ok(QueuedPlayLog {
                id: r.get(0)?,
                uuid: r.get(1)?,
                user_id: r.get::<_, i64>(2)? as u64,
                body: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Removes listens sent, refused for good, or of another account.
    pub fn drop_play_logs(&self, ids: &[i64]) -> Result<()> {
        let mut db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let tx = db.conn.transaction()?;
        {
            let mut delete = tx.prepare("DELETE FROM play_logs WHERE id = ?1")?;
            for id in ids {
                delete.execute(params![id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    #[cfg(test)]
    pub fn play_log_count(&self) -> Result<u32> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        Ok(db
            .conn
            .query_row("SELECT COUNT(*) FROM play_logs", [], |r| r.get(0))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_read_and_drop() {
        let store = Store::in_memory().unwrap();
        for i in 0..3 {
            store
                .queue_play_log(&format!("u{i}"), 7, &format!("{{\"n\":{i}}}"))
                .unwrap();
        }
        let first = store.pending_play_logs(2).unwrap();
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].uuid, "u0");
        assert_eq!(first[0].user_id, 7);
        assert_eq!(first[1].body, "{\"n\":1}");
        store
            .drop_play_logs(&first.iter().map(|l| l.id).collect::<Vec<_>>())
            .unwrap();
        let rest = store.pending_play_logs(10).unwrap();
        assert_eq!(rest.len(), 1);
        assert_eq!(rest[0].uuid, "u2");
        assert_eq!(store.play_log_count().unwrap(), 1);
    }

    #[test]
    fn oldest_are_dropped_past_the_cap() {
        let store = Store::in_memory().unwrap();
        for i in 0..MAX_PLAY_LOGS + 3 {
            store.queue_play_log(&i.to_string(), 1, "{}").unwrap();
        }
        assert_eq!(store.play_log_count().unwrap(), MAX_PLAY_LOGS as u32);
        assert_eq!(store.pending_play_logs(1).unwrap()[0].uuid, "3");
    }
}
