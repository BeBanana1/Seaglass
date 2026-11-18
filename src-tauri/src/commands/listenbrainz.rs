//! Réglages › Application › ListenBrainz (#431) : compte connecté, connexion par jeton,
//! retrait. Tout passe par `integrations::listenbrainz` ; l'UI ne parle jamais au service.

use super::CmdResult;
use crate::integrations::listenbrainz::{self, Status};

#[tauri::command]
pub async fn listenbrainz_status() -> CmdResult<Status> {
    Ok(listenbrainz::status().await)
}

/// Vérifie le jeton collé auprès de ListenBrainz avant de le garder.
#[tauri::command]
pub async fn listenbrainz_connect(token: String) -> CmdResult<Status> {
    listenbrainz::connect(token).await
}

#[tauri::command]
pub async fn listenbrainz_disconnect() -> CmdResult<Status> {
    listenbrainz::disconnect().await
}
