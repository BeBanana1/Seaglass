//! Commandes du lecteur : toutes deviennent un `Cmd` envoyé à l'acteur.

use tauri::State;
use tokio::sync::oneshot;

use super::{AppState, CmdResult};
use crate::player::{Cmd, PlayerState, QueueView, Repeat};
use crate::tidal::models::TrackId;

const MAX_CONTEXT_LEN: usize = 10_000;

/// Lit `track_ids` comme contexte (album, playlist, résultats de recherche) à partir de
/// `start_index`. Avec `shuffle`, active la lecture aléatoire et part d'un titre au hasard.
#[tauri::command]
pub async fn play_tracks(
    state: State<'_, AppState>,
    track_ids: Vec<u64>,
    start_index: usize,
    shuffle: Option<bool>,
) -> CmdResult<()> {
    if track_ids.is_empty() || track_ids.len() > MAX_CONTEXT_LEN {
        return Err(format!(
            "le contexte doit contenir entre 1 et {MAX_CONTEXT_LEN} titres"
        ));
    }
    if start_index >= track_ids.len() {
        return Err("index de départ hors du contexte".into());
    }
    let ids = track_ids.into_iter().map(TrackId).collect();
    send_cmd(
        &state,
        Cmd::PlayTracks {
            ids,
            start: start_index,
            shuffle: shuffle.unwrap_or(false),
        },
    )
    .await
}

#[tauri::command]
pub async fn play_next(state: State<'_, AppState>, track_id: u64) -> CmdResult<()> {
    send_cmd(&state, Cmd::PlayNext(TrackId(track_id))).await
}

#[tauri::command]
pub async fn add_to_queue(state: State<'_, AppState>, track_id: u64) -> CmdResult<()> {
    send_cmd(&state, Cmd::AddToQueue(TrackId(track_id))).await
}

/// Retire l'entrée `index` de la file manuelle.
#[tauri::command]
pub async fn remove_from_queue(state: State<'_, AppState>, index: usize) -> CmdResult<()> {
    send_cmd(&state, Cmd::RemoveFromQueue(index)).await
}

#[tauri::command]
pub async fn next(state: State<'_, AppState>) -> CmdResult<()> {
    send_cmd(&state, Cmd::Next).await
}

#[tauri::command]
pub async fn previous(state: State<'_, AppState>) -> CmdResult<()> {
    send_cmd(&state, Cmd::Previous).await
}

#[tauri::command]
pub async fn toggle_pause(state: State<'_, AppState>) -> CmdResult<()> {
    send_cmd(&state, Cmd::TogglePause).await
}

#[tauri::command]
pub async fn seek(state: State<'_, AppState>, seconds: f64) -> CmdResult<()> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err("position invalide".into());
    }
    send_cmd(&state, Cmd::Seek(seconds)).await
}

#[tauri::command]
pub async fn set_shuffle(state: State<'_, AppState>, enabled: bool) -> CmdResult<()> {
    send_cmd(&state, Cmd::SetShuffle(enabled)).await
}

#[tauri::command]
pub async fn set_repeat(state: State<'_, AppState>, mode: Repeat) -> CmdResult<()> {
    send_cmd(&state, Cmd::SetRepeat(mode)).await
}

#[tauri::command]
pub async fn player_state(state: State<'_, AppState>) -> CmdResult<PlayerState> {
    let (tx, rx) = oneshot::channel();
    send_cmd(&state, Cmd::GetState(tx)).await?;
    rx.await.map_err(|_| "lecteur arrêté".to_owned())
}

#[tauri::command]
pub async fn queue_state(state: State<'_, AppState>) -> CmdResult<QueueView> {
    let (tx, rx) = oneshot::channel();
    send_cmd(&state, Cmd::GetQueue(tx)).await?;
    rx.await.map_err(|_| "lecteur arrêté".to_owned())
}

async fn send_cmd(state: &AppState, cmd: Cmd) -> CmdResult<()> {
    state
        .player
        .send(cmd)
        .await
        .map_err(|_| "lecteur arrêté".to_owned())
}
