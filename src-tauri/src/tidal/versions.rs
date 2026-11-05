//! Dernière version connue de chaque playlist modifiée dans l'app (#201).
//!
//! L'API des playlists est cohérente à terme : juste après une écriture, `playlists/{uuid}`
//! peut encore renvoyer l'ancien ETag, et une écriture envoyée avec cet ETag à une réplique
//! en retard y est acceptée et appliquée sur l'ancien état (vu le 2026-11-05 : un
//! déplacement appliqué avant l'ajout qui le précédait). L'ETag vaut `lastUpdated` en ms
//! (`"1793747577268"`) : on retient le plus récent vu après chaque écriture, et un ETag plus
//! ancien est relu au lieu d'être envoyé.

use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
pub struct PlaylistVersions {
    known: Mutex<HashMap<String, u64>>,
}

impl PlaylistVersions {
    /// L'ETag est plus ancien qu'une version déjà vue : une réplique en retard a répondu.
    pub fn is_stale(&self, uuid: &str, etag: &str) -> bool {
        let (Some(version), Ok(known)) = (parse(etag), self.known.lock()) else {
            return false;
        };
        known.get(uuid).is_some_and(|&k| version < k)
    }

    /// Retient cette version si elle est plus récente que la dernière connue.
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

/// `"1793747577268"` ou `W/"…"` → 1793747577268 ; autre forme → `None` (jamais périmé).
pub fn parse(etag: &str) -> Option<u64> {
    etag.trim_start_matches("W/").trim_matches('"').parse().ok()
}

/// `after` est une version postérieure à `before` (écriture visible).
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
        // Forme inconnue : jamais bloquante.
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
