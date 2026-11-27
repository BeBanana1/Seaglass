//! Last known version of each playlist modified in the app (#201).
//!
//! The playlists API is eventually consistent: right after a write, `playlists/{uuid}` may
//! still return the old ETag, and a write sent with that ETag to a lagging replica is
//! accepted there and applied on the old state (seen on 2026-11-05: a move applied before
//! the add that preceded it). The ETag is `lastUpdated` in ms (`"1793747577268"`): the most
//! recent one seen after each write is kept, and an older ETag is read again instead of
//! being sent.

use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
pub struct PlaylistVersions {
    known: Mutex<HashMap<String, u64>>,
}

impl PlaylistVersions {
    /// The ETag is older than a version already seen: a lagging replica answered.
    pub fn is_stale(&self, uuid: &str, etag: &str) -> bool {
        let (Some(version), Ok(known)) = (parse(etag), self.known.lock()) else {
            return false;
        };
        known.get(uuid).is_some_and(|&k| version < k)
    }

    /// Keeps this version if it is more recent than the last known one.
    pub fn note(&self, uuid: &str, etag: &str) {
        let (Some(version), Ok(mut known)) = (parse(etag), self.known.lock()) else {
            return;
        };
        let entry = known.entry(uuid.to_owned()).or_insert(version);
        *entry = (*entry).max(version);
    }

    pub fn clear(&self) {
        if let Ok(mut known) = self.known.lock() {
            known.clear();
        }
    }
}

/// `"1793747577268"` or `W/"…"` → 1793747577268; other form → `None` (never stale).
pub fn parse(etag: &str) -> Option<u64> {
    etag.trim_start_matches("W/").trim_matches('"').parse().ok()
}

/// `after` is a version later than `before` (write visible).
pub fn is_newer(after: &str, before: &str) -> bool {
    match (parse(after), parse(before)) {
        (Some(a), Some(b)) => a > b,
        _ => after != before,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_etags_are_stale() {
        let versions = PlaylistVersions::default();
        assert!(!versions.is_stale("a", "\"100\""));
        versions.note("a", "\"100\"");
        versions.note("a", "\"90\"");
        assert!(versions.is_stale("a", "\"99\""));
        assert!(!versions.is_stale("a", "\"100\""));
        assert!(!versions.is_stale("a", "\"101\""));
        assert!(!versions.is_stale("b", "\"1\""));
        // Unknown form: never blocking.
        assert!(!versions.is_stale("a", "abc"));
        versions.clear();
        assert!(!versions.is_stale("a", "\"1\""));
    }

    #[test]
    fn etag_forms() {
        assert_eq!(parse("\"1793747577268\""), Some(1_793_747_577_268));
        assert_eq!(parse("W/\"12\""), Some(12));
        assert_eq!(parse("x"), None);
        assert!(is_newer("\"2\"", "\"1\""));
        assert!(!is_newer("\"1\"", "\"1\""));
        assert!(is_newer("b", "a"));
    }
}
