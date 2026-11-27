//! Player commands: all of them become a `Cmd` sent to the actor.

use tauri::State;
use tokio::sync::oneshot;

use super::settings::pins::is_uuid;
use super::{AppState, CmdResult};
use crate::i18n::{tr, trf};
use crate::player::sleep::SleepRequest;
use crate::player::volume::Volume;
use crate::player::{Cmd, PlaySource, PlayerState, QueuePart, QueueView, Repeat};
use crate::tidal::models::TrackId;

const MAX_CONTEXT_LEN: usize = 10_000;
// Playing or shuffling a whole library category must stay accepted (#91).
const _: () = assert!(crate::tidal::LIBRARY_MAX <= MAX_CONTEXT_LEN);

/// Plays `track_ids` as the context (album, playlist, search results) from `start_index`.
/// `shuffle`: `true` turns shuffle on and starts on a random track, `false` turns it off
/// ("Play" buttons, #526), absent keeps the current mode. `source` (#124) is only displayed:
/// an invalid one is dropped without refusing playback.
#[tauri::command]
pub async fn play_tracks(
    state: State<'_, AppState>,
    track_ids: Vec<u64>,
    start_index: usize,
    shuffle: Option<bool>,
    source: Option<PlaySource>,
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
            shuffle,
            source: source.and_then(|s| s.sanitize(is_uuid)),
        },
    )
    .await
}

/// True if a tap on the Android notification asked for "Now playing" since the last
/// call (#163); always false on desktop. Read at UI startup and on the
/// `open-now-playing` event.
#[tauri::command]
pub async fn take_now_playing_request() -> CmdResult<bool> {
    Ok(crate::integrations::take_now_playing_request())
}

#[tauri::command]
pub async fn play_next(state: State<'_, AppState>, track_id: u64) -> CmdResult<()> {
    send_cmd(&state, Cmd::PlayNext(TrackId(track_id))).await
}

#[tauri::command]
pub async fn add_to_queue(state: State<'_, AppState>, track_id: u64) -> CmdResult<()> {
    send_cmd(&state, Cmd::AddToQueue(TrackId(track_id))).await
}

/// Adds several tracks (album, playlist) to the manual queue, in the given order:
/// "Play next" with `next`, otherwise "Add to queue".
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

/// Removes entry `index` from the manual queue.
#[tauri::command]
pub async fn remove_from_queue(state: State<'_, AppState>, index: usize) -> CmdResult<()> {
    send_cmd(&state, Cmd::RemoveFromQueue(index)).await
}

/// Moves entry `from` of the manual queue to index `to` (drag and drop, Move up /
/// Move down). An index outside the queue is ignored by the actor.
#[tauri::command]
pub async fn move_in_queue(state: State<'_, AppState>, from: usize, to: usize) -> CmdResult<()> {
    send_cmd(&state, Cmd::MoveInQueue { from, to }).await
}

/// Plays track `index` of the manual queue or of the rest of the context, as
/// `queue-state` describes them (#167). `track_id` is the displayed track: if the queue has
/// changed since, nothing happens.
#[tauri::command]
pub async fn play_from_queue(
    state: State<'_, AppState>,
    part: QueuePart,
    index: usize,
    track_id: u64,
) -> CmdResult<()> {
    let id = TrackId(track_id);
    send_cmd(&state, Cmd::PlayFromQueue { part, index, id }).await
}

/// Clears the queue (manual and rest of the context) without stopping the current track.
#[tauri::command]
pub async fn clear_queue(state: State<'_, AppState>) -> CmdResult<()> {
    send_cmd(&state, Cmd::ClearQueue).await
}

/// Undoes the last "Clear queue" or removal of a track from the queue (#349). `false`:
/// the queue has changed since, there is nothing left to undo.
#[tauri::command]
pub async fn restore_queue(state: State<'_, AppState>) -> CmdResult<bool> {
    let (tx, rx) = oneshot::channel();
    send_cmd(&state, Cmd::RestoreQueue(tx)).await?;
    rx.await
        .map_err(|_| crate::i18n::tr("lecteur arrêté", "player stopped").to_owned())
}

/// Answer to the "Hi-res edition available" banner (#59), valid for the whole album.
#[tauri::command]
pub async fn answer_hires_offer(state: State<'_, AppState>, accept: bool) -> CmdResult<()> {
    send_cmd(&state, Cmd::AnswerHiresOffer(accept)).await
}

/// Sleep timer (#85): pause after `minutes` or at the end of the current track;
/// `null` cancels it.
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

/// Software volume from 0 to 100 (100 = unity gain, the only bit-perfect setting).
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

/// Tracks of the queue in play order (#327), to save it as a playlist:
/// the UI only receives the start of the rest. Capped like an addition to a playlist.
#[tauri::command]
pub async fn queue_track_ids(state: State<'_, AppState>) -> CmdResult<Vec<TrackId>> {
    let (tx, rx) = oneshot::channel();
    send_cmd(&state, Cmd::GetPlayOrder(super::collection::MAX_ADD, tx)).await?;
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
