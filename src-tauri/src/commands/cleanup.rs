//! "Clean up" a user's playlist or their liked tracks (#382): report, then
//! changes chosen by the user, and their undo.

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
        return Err(format!("at most {MAX_ADD} changes"));
    }
    Ok(())
}

/// Duplicates and unavailable tracks of one of your playlists, with their replacements.
#[tauri::command]
pub async fn playlist_cleanup(
    state: State<'_, AppState>,
    uuid: String,
) -> CmdResult<CleanupReport> {
    check_uuid(&uuid)?;
    state.tidal.playlist_cleanup(&uuid).await.map_err(err)
}

/// Applies the chosen changes (positions in the report). Also returns what is needed to undo.
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

/// Undoes a cleanup: `ops` = the `undo` returned by `clean_playlist`, applied as is.
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

/// Duplicates and unavailable tracks of the liked tracks.
#[tauri::command]
pub async fn liked_cleanup(state: State<'_, AppState>) -> CmdResult<CleanupReport> {
    state.tidal.liked_cleanup().await.map_err(err)
}

/// Applies changes to the liked tracks (or undoes them: order does not matter there).
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
