//! Blocked tracks and artists (#334): suggested neither by mixes nor by radios.

use tauri::State;

use super::{AppState, CmdResult, err};
use crate::tidal::models::{BlockTarget, Blocked};

/// Both lists, read from TIDAL (Settings › Playback).
#[tauri::command]
pub async fn blocked(state: State<'_, AppState>) -> CmdResult<Blocked> {
    state.tidal.blocked().await.map_err(err)
}

/// Blocks or unblocks a track or an artist.
#[tauri::command]
pub async fn set_blocked(
    state: State<'_, AppState>,
    target: BlockTarget,
    blocked: bool,
) -> CmdResult<()> {
    state.tidal.set_blocked(&target, blocked).await.map_err(err)
}
