//! Jam locale (#25), bureau seulement : démarrer, arrêter, état.

use tauri::State;

use super::{AppState, CmdResult};
use crate::jam::{JamInfo, JamState};

/// Jam en cours, ou `null`.
#[tauri::command]
pub async fn jam_status(jam: State<'_, JamState>) -> CmdResult<Option<JamInfo>> {
    Ok(jam.info())
}

/// Démarre la Jam (ou renvoie celle qui tourne) : adresse et code à donner aux invités.
#[tauri::command]
pub async fn start_jam(state: State<'_, AppState>, jam: State<'_, JamState>) -> CmdResult<JamInfo> {
    jam.start(state.tidal.clone(), state.player.clone()).await
}

#[tauri::command]
pub async fn stop_jam(jam: State<'_, JamState>) -> CmdResult<()> {
    jam.stop();
    Ok(())
}
