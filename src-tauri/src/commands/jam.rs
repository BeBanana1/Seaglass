//! Jam (#25) : démarrer (en ligne, ou sur le réseau local depuis le bureau), arrêter, état.

use tauri::{AppHandle, State};

use super::{AppState, CmdResult};
use crate::jam::{JamMode, JamState, JamStatus};

/// Jam en cours (adresse, code, QR code, connexion, noms des invités), ou `null`.
#[tauri::command]
pub async fn jam_status(jam: State<'_, JamState>) -> CmdResult<Option<JamStatus>> {
    Ok(jam.status())
}

/// Vrai si l'app peut ouvrir une Jam en ligne (clé du relais compilée).
#[tauri::command]
pub async fn jam_online_available() -> CmdResult<bool> {
    Ok(crate::jam::online_available())
}

/// Démarre la Jam (ou renvoie celle qui tourne) : adresse et code à donner aux invités.
/// En ligne, l'adresse arrive ensuite par l'événement `jam-status`.
#[tauri::command]
pub async fn start_jam(
    app: AppHandle,
    state: State<'_, AppState>,
    jam: State<'_, JamState>,
    mode: JamMode,
) -> CmdResult<JamStatus> {
    jam.start(mode, app, state.tidal.clone(), state.player.clone())
        .await
}

#[tauri::command]
pub async fn stop_jam(jam: State<'_, JamState>) -> CmdResult<()> {
    jam.stop();
    Ok(())
}
