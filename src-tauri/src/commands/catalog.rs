//! Commandes de navigation : recherche, pages album / artiste / playlist / mix, radios,
//! bibliothèque, accueil, fil d'actualité.

use tauri::State;

use super::settings::home_layout::HomeSection;
use super::{AppState, CmdResult, err, settings};
use crate::tidal::models::{
    AlbumPage, AlbumSummary, ArtistPage, ArtistSummary, FeedItem, MixPage, MixSummary,
    PlaylistPage, PlaylistSummary, RadioPage, RadioSeed, SearchResults, TrackEntry,
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
pub async fn mix(state: State<'_, AppState>, id: String) -> CmdResult<MixPage> {
    if !is_mix_id(&id) {
        return Err("identifiant de mix invalide".into());
    }
    state.tidal.mix(&id).await.map_err(err)
}

#[tauri::command]
pub async fn radio(state: State<'_, AppState>, seed: RadioSeed, id: u64) -> CmdResult<RadioPage> {
    state.tidal.radio(seed, id).await.map_err(err)
}

#[tauri::command]
pub async fn library_mixes(state: State<'_, AppState>) -> CmdResult<Vec<MixSummary>> {
    state.tidal.favorite_mixes().await.map_err(err)
}

#[tauri::command]
pub async fn library_tracks(state: State<'_, AppState>) -> CmdResult<Vec<TrackEntry>> {
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

/// Sorties récentes des artistes suivis.
#[tauri::command]
pub async fn feed(state: State<'_, AppState>) -> CmdResult<Vec<FeedItem>> {
    state.tidal.feed().await.map_err(err)
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

/// Identifiant de mix : alphanumérique (30 caractères hexadécimaux observés), il part dans l'URL.
fn is_mix_id(s: &str) -> bool {
    (1..=64).contains(&s.len()) && s.chars().all(|c| c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::{is_mix_id, is_uuid};

    #[test]
    fn mix_id_validation() {
        assert!(is_mix_id("01636e04b6e9ec28485f7d0372b7f3"));
        assert!(!is_mix_id(""));
        assert!(!is_mix_id("../users/1"));
        assert!(!is_mix_id("abc?x=1"));
        assert!(!is_mix_id(&"a".repeat(65)));
    }

    #[test]
    fn uuid_validation() {
        assert!(is_uuid("4ba30155-d807-446f-9e53-b67520ccfe83"));
        assert!(!is_uuid("4ba30155-d807-446f-9e53"));
        assert!(!is_uuid("../../users/1-d807-446f-9e53-b67520ccfe83"));
        assert!(!is_uuid("4ba30155-d807-446f-9e53-b67520ccfe8z"));
    }
}
