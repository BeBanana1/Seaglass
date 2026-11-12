//! Titres et artistes bloqués (#334) : ni proposés par les mixes ni par les radios.

use tauri::State;

use super::{AppState, CmdResult, err};
use crate::tidal::models::{BlockTarget, Blocked};

/// Les deux listes, lues chez TIDAL (Réglages › Lecture).
#[tauri::command]
pub async fn blocked(state: State<'_, AppState>) -> CmdResult<Blocked> {
    state.tidal.blocked().await.map_err(err)
}

/// Bloque ou débloque un titre ou un artiste.
#[tauri::command]
pub async fn set_blocked(
    state: State<'_, AppState>,
    target: BlockTarget,
    blocked: bool,
) -> CmdResult<()> {
    state.tidal.set_blocked(&target, blocked).await.map_err(err)
}
