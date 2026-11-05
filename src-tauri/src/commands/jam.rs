//! Jam locale (#25), bureau seulement : démarrer, arrêter, état.

use tauri::{AppHandle, State};

use super::{AppState, CmdResult};
use crate::jam::{JamState, JamStatus};

/// Jam en cours (adresse, code, QR code, noms des invités), ou `null`.
#[tauri::command]
pub async fn jam_status(jam: State<'_, JamState>) -> CmdResult<Option<JamStatus>> {
    Ok(jam.status())
}

/// Démarre la Jam (ou renvoie celle qui tourne) : adresse et code à donner aux invités.
#[tauri::command]
pub async fn start_jam(
    app: AppHandle,
    state: State<'_, AppState>,
    jam: State<'_, JamState>,
) -> CmdResult<JamStatus> {
    jam.start(app, state.tidal.clone(), state.player.clone())
        .await
}

#[tauri::command]
pub async fn stop_jam(jam: State<'_, JamState>) -> CmdResult<()> {
    jam.stop();
    Ok(())
}
