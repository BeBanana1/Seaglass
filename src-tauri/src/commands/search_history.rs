//! Historique de recherche (#211) : dernières requêtes, la plus récente en tête, dans la
//! table `kv` (`search.history`). L'UI cherche pendant la frappe (#248) : elle n'enregistre
//! une requête (`note_search_history`) que lorsqu'elle est retenue (Entrée, clic dans les
//! résultats), sinon chaque préfixe tapé finirait dans la liste.
//! Effacé à la déconnexion (il appartient à la personne connectée).

use tauri::State;

use super::{AppState, CmdResult, err};
use crate::store::Store;

const KEY: &str = "search.history";

/// Nombre de requêtes gardées ; au-delà, la plus ancienne est oubliée.
pub const MAX_ENTRIES: usize = 20;

/// Forme gardée d'une requête : espaces de bord retirés, espaces intérieurs réduits à un.
fn normalize(query: &str) -> String {
    query.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn same(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// Met `query` en tête ; une requête identique (casse ignorée) est remplacée par la
/// nouvelle forme. Une requête vide n'est pas gardée.
pub fn record(history: &mut Vec<String>, query: &str) {
    let query = normalize(query);
    if query.is_empty() {
        return;
    }
    history.retain(|q| !same(q, &query));
    history.insert(0, query);
    history.truncate(MAX_ENTRIES);
}

/// Retire `query` (casse et espaces ignorés).
pub fn remove(history: &mut Vec<String>, query: &str) {
    let query = normalize(query);
    history.retain(|q| !same(q, &query));
}

fn load(store: Option<&Store>) -> Vec<String> {
    store
        .and_then(|s| s.get_setting::<Vec<String>>(KEY).ok().flatten())
        .unwrap_or_default()
}

fn save(store: Option<&Store>, history: &[String]) -> CmdResult<()> {
    match store {
        Some(store) => store.set_setting(KEY, &history).map_err(err),
        None => Ok(()),
    }
}

/// Déconnexion : l'historique de la personne précédente disparaît.
pub(super) fn clear(state: &AppState) {
    if let Err(e) = save(state.store.as_deref(), &[]) {
        tracing::warn!("historique de recherche non effacé : {e}");
    }
}

#[tauri::command]
pub async fn search_history(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    Ok(load(state.store.as_deref()))
}

/// Enregistre une requête retenue ; renvoie l'historique à jour.
#[tauri::command]
pub async fn note_search_history(
    state: State<'_, AppState>,
    query: String,
) -> CmdResult<Vec<String>> {
    if query.chars().count() > super::catalog::MAX_QUERY_LEN {
        return Ok(load(state.store.as_deref()));
    }
    let store = state.store.as_deref();
    let mut history = load(store);
    record(&mut history, &query);
    save(store, &history)?;
    Ok(history)
}

/// Retire une requête ; renvoie l'historique restant.
#[tauri::command]
pub async fn remove_search_history(
    state: State<'_, AppState>,
    query: String,
) -> CmdResult<Vec<String>> {
    let store = state.store.as_deref();
    let mut history = load(store);
    remove(&mut history, &query);
    save(store, &history)?;
    Ok(history)
}

#[tauri::command]
pub async fn clear_search_history(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    save(state.store.as_deref(), &[])?;
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_first_without_duplicates() {
        let mut h = Vec::new();
        record(&mut h, "daft punk");
        record(&mut h, "  Air   Moon  Safari ");
        record(&mut h, "Daft Punk");
        assert_eq!(h, ["Daft Punk", "Air Moon Safari"]);
    }

    #[test]
    fn empty_query_ignored() {
        let mut h = vec!["air".to_owned()];
        record(&mut h, "   ");
        assert_eq!(h, ["air"]);
    }

    #[test]
    fn oldest_dropped_beyond_limit() {
        let mut h = Vec::new();
        for i in 0..=MAX_ENTRIES {
            record(&mut h, &format!("q{i}"));
        }
        assert_eq!(h.len(), MAX_ENTRIES);
        assert_eq!(h[0], format!("q{MAX_ENTRIES}"));
        assert!(!h.contains(&"q0".to_owned()));
    }

    #[test]
    fn remove_ignores_case_and_spaces() {
        let mut h = vec!["Daft Punk".to_owned(), "air".to_owned()];
        remove(&mut h, " daft  punk ");
        assert_eq!(h, ["air"]);
        remove(&mut h, "absent");
        assert_eq!(h, ["air"]);
    }
}
