//! Commandes du lecteur : toutes deviennent un `Cmd` envoyé à l'acteur.

use tauri::State;
use tokio::sync::oneshot;

use super::{AppState, CmdResult};
use crate::i18n::{tr, trf};
use crate::player::sleep::SleepRequest;
use crate::player::volume::Volume;
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
        return Err(trf!(
            "le contexte doit contenir entre 1 et {MAX_CONTEXT_LEN} titres",
            "the context must contain 1 to {MAX_CONTEXT_LEN} tracks"
        ));
    }
    if start_index >= track_ids.len() {
        return Err(tr(
            "index de départ hors du contexte",
            "start index out of context",
        )
        .into());
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

/// Ajoute plusieurs titres (album, playlist) à la file manuelle, dans l'ordre donné :
/// « Lire ensuite » avec `next`, sinon « Ajouter à la file ».
#[tauri::command]
pub async fn enqueue_tracks(
    state: State<'_, AppState>,
    track_ids: Vec<u64>,
    next: bool,
) -> CmdResult<()> {
    if track_ids.is_empty() || track_ids.len() > MAX_CONTEXT_LEN {
        return Err(trf!(
            "la liste doit contenir entre 1 et {MAX_CONTEXT_LEN} titres",
            "the list must contain 1 to {MAX_CONTEXT_LEN} tracks"
        ));
    }
    let ids = track_ids.into_iter().map(TrackId).collect();
    send_cmd(&state, Cmd::Enqueue { ids, next }).await
}

/// Retire l'entrée `index` de la file manuelle.
#[tauri::command]
pub async fn remove_from_queue(state: State<'_, AppState>, index: usize) -> CmdResult<()> {
    send_cmd(&state, Cmd::RemoveFromQueue(index)).await
}

/// Vide la file (manuelle et suite du contexte) sans arrêter le titre en cours.
#[tauri::command]
pub async fn clear_queue(state: State<'_, AppState>) -> CmdResult<()> {
    send_cmd(&state, Cmd::ClearQueue).await
}

/// Réponse au bandeau « Édition hi-res disponible » (#59), valable pour tout l'album.
#[tauri::command]
pub async fn answer_hires_offer(state: State<'_, AppState>, accept: bool) -> CmdResult<()> {
    send_cmd(&state, Cmd::AnswerHiresOffer(accept)).await
}

/// Minuteur de mise en veille (#85) : pause après `minutes` ou à la fin du titre en cours ;
/// `null` l'annule.
#[tauri::command]
pub async fn set_sleep_timer(
    state: State<'_, AppState>,
    timer: Option<SleepRequest>,
) -> CmdResult<()> {
    let timer = timer.map(SleepRequest::validate).transpose()?;
    send_cmd(&state, Cmd::SetSleepTimer(timer)).await
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
        return Err(tr("position invalide", "invalid position").into());
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

/// Volume logiciel de 0 à 100 (100 = gain unitaire, seul réglage bit-perfect).
#[tauri::command]
pub async fn set_volume(state: State<'_, AppState>, volume: u8) -> CmdResult<()> {
    let level = Volume::validate(volume).map_err(|e| e.to_string())?;
    send_cmd(&state, Cmd::SetVolume(level)).await
}

#[tauri::command]
pub async fn set_muted(state: State<'_, AppState>, muted: bool) -> CmdResult<()> {
    send_cmd(&state, Cmd::SetMuted(muted)).await
}

#[tauri::command]
pub async fn player_state(state: State<'_, AppState>) -> CmdResult<PlayerState> {
    let (tx, rx) = oneshot::channel();
    send_cmd(&state, Cmd::GetState(tx)).await?;
    rx.await
        .map_err(|_| crate::i18n::tr("lecteur arrêté", "player stopped").to_owned())
}

#[tauri::command]
pub async fn queue_state(state: State<'_, AppState>) -> CmdResult<QueueView> {
    let (tx, rx) = oneshot::channel();
    send_cmd(&state, Cmd::GetQueue(tx)).await?;
    rx.await
        .map_err(|_| crate::i18n::tr("lecteur arrêté", "player stopped").to_owned())
}

async fn send_cmd(state: &AppState, cmd: Cmd) -> CmdResult<()> {
    state
        .player
        .send(cmd)
        .await
        .map_err(|_| crate::i18n::tr("lecteur arrêté", "player stopped").to_owned())
}
