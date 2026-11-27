//! Settings › Application › ListenBrainz (#431): connected account, sign-in by token,
//! removal. Everything goes through `integrations::listenbrainz`; the UI never talks to the
//! service.

use super::CmdResult;
use crate::integrations::listenbrainz::{self, Status};

#[tauri::command]
pub async fn listenbrainz_status() -> CmdResult<Status> {
    Ok(listenbrainz::status().await)
}

/// Checks the pasted token with ListenBrainz before keeping it.
#[tauri::command]
pub async fn listenbrainz_connect(token: String) -> CmdResult<Status> {
    listenbrainz::connect(token).await
}

#[tauri::command]
pub async fn listenbrainz_disconnect() -> CmdResult<Status> {
    listenbrainz::disconnect().await
}
