//! « Nettoyer » une playlist de l'utilisateur ou ses titres likés (#382) : rapport, puis
//! changements choisis par l'utilisateur, et leur annulation.

use tauri::{AppHandle, State};

use super::catalog::is_uuid;
use super::collection::{MAX_ADD, emit_likes};
use super::{AppState, CmdResult, err};
use crate::tidal::models::{CleanupReport, ListOp, ListOpsResult};

fn check_uuid(uuid: &str) -> CmdResult<()> {
    if is_uuid(uuid) {
        Ok(())
    } else {
        Err("identifiant de playlist invalide".into())
    }
}

fn check_len(ops: &[ListOp]) -> CmdResult<()> {
    if ops.len() > MAX_ADD {
        return Err(format!("{MAX_ADD} changements au plus"));
    }
    Ok(())
}

/// Doublons et titres indisponibles d'une de vos playlists, avec leurs remplaçants.
#[tauri::command]
pub async fn playlist_cleanup(
    state: State<'_, AppState>,
    uuid: String,
) -> CmdResult<CleanupReport> {
    check_uuid(&uuid)?;
    state.tidal.playlist_cleanup(&uuid).await.map_err(err)
}

/// Applique les changements choisis (positions du rapport). Renvoie aussi de quoi annuler.
#[tauri::command]
pub async fn clean_playlist(
    state: State<'_, AppState>,
    uuid: String,
    ops: Vec<ListOp>,
) -> CmdResult<ListOpsResult> {
    check_uuid(&uuid)?;
    check_len(&ops)?;
    super::snapshots::forget_playlist(&state, &uuid);
    state.tidal.clean_playlist(&uuid, ops).await.map_err(err)
}

/// Annule un nettoyage : `ops` = le `undo` renvoyé par `clean_playlist`, appliqué tel quel.
#[tauri::command]
pub async fn undo_playlist_cleanup(
    state: State<'_, AppState>,
    uuid: String,
    ops: Vec<ListOp>,
) -> CmdResult<ListOpsResult> {
    check_uuid(&uuid)?;
    check_len(&ops)?;
    super::snapshots::forget_playlist(&state, &uuid);
    Ok(state.tidal.apply_playlist_ops(&uuid, &ops).await)
}

/// Doublons et titres indisponibles des titres likés.
#[tauri::command]
pub async fn liked_cleanup(state: State<'_, AppState>) -> CmdResult<CleanupReport> {
    state.tidal.liked_cleanup().await.map_err(err)
}

/// Applique des changements aux titres likés (ou les annule : l'ordre n'y compte pas).
#[tauri::command]
pub async fn clean_liked(
    app: AppHandle,
    state: State<'_, AppState>,
    ops: Vec<ListOp>,
) -> CmdResult<ListOpsResult> {
    check_len(&ops)?;
    let result = state.tidal.apply_liked_ops(&ops).await;
    if result.applied > 0 {
        emit_likes(&app, &state).await;
    }
    Ok(result)
}
