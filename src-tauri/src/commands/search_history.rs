//! Search history (#211): latest queries, most recent first, in the `kv` table
//! (`search.history`). The UI searches while typing (#248): it only records a query
//! (`note_search_history`) once it is retained (Enter, click in the results),
//! otherwise every typed prefix would end up in the list.
//! Cleared at sign-out (it belongs to the signed-in person).

use tauri::State;

use super::{AppState, CmdResult, err};
use crate::store::Store;

const KEY: &str = "search.history";

/// Number of queries kept; beyond it, the oldest is forgotten.
pub const MAX_ENTRIES: usize = 20;

/// Kept form of a query: edge whitespace removed, inner whitespace reduced to one.
fn normalize(query: &str) -> String {
    query.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn same(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// Puts `query` first; an identical query (case ignored) is replaced by the
/// new form. An empty query is not kept.
pub fn record(history: &mut Vec<String>, query: &str) {
    let query = normalize(query);
    if query.is_empty() {
        return;
    }
    history.retain(|q| !same(q, &query));
    history.insert(0, query);
    history.truncate(MAX_ENTRIES);
}

/// Removes `query` (case and whitespace ignored).
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

/// Sign-out: the previous person's history disappears.
pub(super) fn clear(state: &AppState) {
    if let Err(e) = save(state.store.as_deref(), &[]) {
        tracing::warn!("search history not cleared: {e}");
    }
}

#[tauri::command]
pub async fn search_history(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    Ok(load(state.store.as_deref()))
}

/// Records a retained query; returns the up to date history.
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

/// Removes a query; returns the remaining history.
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
