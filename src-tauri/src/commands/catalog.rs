//! Commandes de navigation : recherche, pages album / artiste / playlist, bibliothèque,
//! accueil.

use tauri::State;

use super::{AppState, CmdResult, err, settings};
use crate::home_layout::HomeSection;
use crate::tidal::models::{
    AlbumPage, AlbumSummary, ArtistPage, ArtistSummary, PlaylistPage, PlaylistSummary,
    SearchResults, Track,
};

const MAX_QUERY_LEN: usize = 200;
const SEARCH_LIMIT: u32 = 50;

#[tauri::command]
pub async fn search(state: State<'_, AppState>, query: String) -> CmdResult<SearchResults> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(SearchResults::default());
    }
    if query.chars().count() > MAX_QUERY_LEN {
        return Err(format!("recherche limitée à {MAX_QUERY_LEN} caractères"));
    }
    state.tidal.search(query, SEARCH_LIMIT).await.map_err(err)
}

#[tauri::command]
pub async fn album(state: State<'_, AppState>, id: u64) -> CmdResult<AlbumPage> {
    state.tidal.album(id).await.map_err(err)
}

#[tauri::command]
pub async fn artist(state: State<'_, AppState>, id: u64) -> CmdResult<ArtistPage> {
    state.tidal.artist(id).await.map_err(err)
}

#[tauri::command]
pub async fn playlist(state: State<'_, AppState>, uuid: String) -> CmdResult<PlaylistPage> {
    if !is_uuid(&uuid) {
        return Err("identifiant de playlist invalide".into());
    }
    state.tidal.playlist(&uuid).await.map_err(err)
}

#[tauri::command]
pub async fn library_tracks(state: State<'_, AppState>) -> CmdResult<Vec<Track>> {
    state.tidal.favorite_tracks().await.map_err(err)
}

#[tauri::command]
pub async fn library_albums(state: State<'_, AppState>) -> CmdResult<Vec<AlbumSummary>> {
    state.tidal.favorite_albums().await.map_err(err)
}

#[tauri::command]
pub async fn library_artists(state: State<'_, AppState>) -> CmdResult<Vec<ArtistSummary>> {
    state.tidal.favorite_artists().await.map_err(err)
}

#[tauri::command]
pub async fn library_playlists(state: State<'_, AppState>) -> CmdResult<Vec<PlaylistSummary>> {
    state.tidal.library_playlists().await.map_err(err)
}

/// Lignes de l'accueil dans l'ordre choisi, masquées comprises (marquées `hidden`).
#[tauri::command]
pub async fn home(state: State<'_, AppState>) -> CmdResult<Vec<HomeSection>> {
    let rows = state.tidal.home().await.map_err(err)?;
    Ok(settings::load(&state).home.arrange(rows))
}

/// UUID au format `8-4-4-4-12` (hexadécimal), seul format de playlist accepté dans l'URL.
fn is_uuid(s: &str) -> bool {
    let groups: Vec<&str> = s.split('-').collect();
    groups.len() == 5
        && groups
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(g, len)| g.len() == len && g.chars().all(|c| c.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::is_uuid;

    #[test]
    fn uuid_validation() {
        assert!(is_uuid("4ba30155-d807-446f-9e53-b67520ccfe83"));
        assert!(!is_uuid("4ba30155-d807-446f-9e53"));
        assert!(!is_uuid("../../users/1-d807-446f-9e53-b67520ccfe83"));
        assert!(!is_uuid("4ba30155-d807-446f-9e53-b67520ccfe8z"));
    }
}
