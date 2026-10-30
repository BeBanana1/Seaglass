//! Commandes d'écriture dans la bibliothèque (#4) : likes et playlists de l'utilisateur.

use tauri::{AppHandle, Emitter, State};

use super::catalog::is_uuid;
use super::{AppState, CmdResult, err};
use crate::tidal::models::{PlaylistSummary, TrackId};

const MAX_TITLE_LEN: usize = 200;
/// Titres ajoutés en une fois (un album, une playlist affichée).
const MAX_ADD: usize = 10_000;

/// Événement émis à chaque changement des titres likés, avec la liste complète.
pub const LIKED_TRACKS_EVENT: &str = "liked-tracks";

#[tauri::command]
pub async fn liked_tracks(state: State<'_, AppState>) -> CmdResult<Vec<TrackId>> {
    state.tidal.liked_track_ids().await.map_err(err)
}

#[tauri::command]
pub async fn set_track_liked(
    app: AppHandle,
    state: State<'_, AppState>,
    track_id: TrackId,
    liked: bool,
) -> CmdResult<()> {
    state
        .tidal
        .set_track_liked(track_id, liked)
        .await
        .map_err(err)?;
    emit_likes(&app, &state).await;
    Ok(())
}

/// Les playlists modifiables (créées par l'utilisateur), pour « Ajouter à une playlist ».
#[tauri::command]
pub async fn own_playlists(state: State<'_, AppState>) -> CmdResult<Vec<PlaylistSummary>> {
    state.tidal.own_playlists().await.map_err(err)
}

/// Ajoute les titres à la fin de la playlist ; renvoie le nombre réellement ajoutés
/// (les titres déjà présents sont ignorés).
#[tauri::command]
pub async fn add_to_playlist(
    state: State<'_, AppState>,
    uuid: String,
    track_ids: Vec<TrackId>,
) -> CmdResult<usize> {
    validate_tracks(&uuid, &track_ids)?;
    state
        .tidal
        .add_to_playlist(&uuid, &track_ids)
        .await
        .map_err(err)
}

/// Crée une playlist avec ces titres ; renvoie son uuid.
#[tauri::command]
pub async fn create_playlist(
    state: State<'_, AppState>,
    title: String,
    track_ids: Vec<TrackId>,
) -> CmdResult<String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("titre de playlist vide".into());
    }
    if title.chars().count() > MAX_TITLE_LEN {
        return Err(format!("titre limité à {MAX_TITLE_LEN} caractères"));
    }
    if track_ids.len() > MAX_ADD {
        return Err(format!("{MAX_ADD} titres au plus"));
    }
    let uuid = state.tidal.create_playlist(title, "").await.map_err(err)?;
    if !track_ids.is_empty() {
        state
            .tidal
            .add_to_playlist(&uuid, &track_ids)
            .await
            .map_err(|e| format!("playlist créée, mais ajout des titres impossible : {e}"))?;
    }
    Ok(uuid)
}

/// Retire le titre `track_id` à la position `index` (0 = premier) ; refusé si la playlist
/// a changé depuis son affichage.
#[tauri::command]
pub async fn remove_from_playlist(
    state: State<'_, AppState>,
    uuid: String,
    index: usize,
    track_id: TrackId,
) -> CmdResult<()> {
    if !is_uuid(&uuid) {
        return Err("identifiant de playlist invalide".into());
    }
    state
        .tidal
        .remove_from_playlist(&uuid, index, track_id)
        .await
        .map_err(err)
}

/// Après une lecture complète des favoris, les cœurs peuvent avoir changé.
pub(super) async fn emit_likes(app: &AppHandle, state: &AppState) {
    match state.tidal.liked_track_ids().await {
        Ok(ids) => {
            if let Err(e) = app.emit(LIKED_TRACKS_EVENT, ids) {
                tracing::warn!("émission de {LIKED_TRACKS_EVENT} impossible : {e}");
            }
        }
        Err(e) => tracing::warn!("likes illisibles : {e}"),
    }
}

fn validate_tracks(uuid: &str, track_ids: &[TrackId]) -> Result<(), String> {
    if !is_uuid(uuid) {
        return Err("identifiant de playlist invalide".into());
    }
    if track_ids.is_empty() {
        return Err("aucun titre à ajouter".into());
    }
    if track_ids.len() > MAX_ADD {
        return Err(format!("{MAX_ADD} titres au plus"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "4ba30155-d807-446f-9e53-b67520ccfe83";

    #[test]
    fn track_validation() {
        assert!(validate_tracks(UUID, &[TrackId(1)]).is_ok());
        assert!(validate_tracks(UUID, &[]).is_err());
        assert!(validate_tracks("../users/1", &[TrackId(1)]).is_err());
        assert!(validate_tracks(UUID, &vec![TrackId(1); MAX_ADD + 1]).is_err());
    }
}
