//! Commandes Tauri : valident l'entrée puis délèguent à `tidal` ou envoient un `Cmd`.

use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tokio::sync::{mpsc, oneshot};

use crate::player::{Cmd, PlayerState, QueueView, Repeat};
use crate::tidal::models::{SessionInfo, Track, TrackId};
use crate::tidal::{TidalClient, TidalError, auth};

const LOGIN_WINDOW: &str = "login";
const MAX_QUERY_LEN: usize = 200;
const SEARCH_LIMIT: u32 = 50;
const MAX_CONTEXT_LEN: usize = 10_000;

pub struct AppState {
    pub tidal: Arc<TidalClient>,
    pub player: mpsc::Sender<Cmd>,
}

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
pub async fn auth_status(state: State<'_, AppState>) -> CmdResult<Option<SessionInfo>> {
    state.tidal.current_session().await.map_err(err)
}

/// Ouvre la page de connexion TIDAL dans une fenêtre dédiée et intercepte la redirection
/// finale pour récupérer le code PKCE. La fenêtre n'a aucune capability Tauri.
#[tauri::command]
pub async fn login(app: AppHandle, state: State<'_, AppState>) -> CmdResult<SessionInfo> {
    if let Some(existing) = app.get_webview_window(LOGIN_WINDOW) {
        let _ = existing.destroy();
    }
    let (challenge, url) = state.tidal.begin_login().map_err(err)?;

    let (tx, rx) = oneshot::channel::<Option<String>>();
    let tx = Arc::new(Mutex::new(Some(tx)));
    let send = |tx: &Arc<Mutex<Option<oneshot::Sender<Option<String>>>>>, code| {
        if let Some(tx) = tx.lock().ok().and_then(|mut t| t.take()) {
            let _ = tx.send(code);
        }
    };

    let nav_tx = Arc::clone(&tx);
    let window = WebviewWindowBuilder::new(&app, LOGIN_WINDOW, WebviewUrl::External(url))
        .title("Connexion")
        .inner_size(480.0, 760.0)
        .on_navigation(move |url| match auth::redirect_code(url) {
            Some(code) => {
                send(&nav_tx, Some(code));
                false
            }
            None => true,
        })
        .build()
        .map_err(err)?;
    let close_tx = Arc::clone(&tx);
    window.on_window_event(move |ev| {
        if matches!(
            ev,
            WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed
        ) {
            send(&close_tx, None);
        }
    });

    let code = rx.await.ok().flatten();
    let _ = window.destroy();
    let code = code.ok_or_else(|| err(TidalError::LoginCancelled))?;
    state
        .tidal
        .complete_login(&challenge, &code)
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn logout(state: State<'_, AppState>) -> CmdResult<()> {
    state.tidal.logout().await.map_err(err)
}

#[tauri::command]
pub async fn search(state: State<'_, AppState>, query: String) -> CmdResult<Vec<Track>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    if query.chars().count() > MAX_QUERY_LEN {
        return Err(format!("recherche limitée à {MAX_QUERY_LEN} caractères"));
    }
    state
        .tidal
        .search_tracks(query, SEARCH_LIMIT)
        .await
        .map_err(err)
}

/// Lit `track_ids` comme contexte (album, playlist, résultats de recherche) à partir de
/// `start_index`.
#[tauri::command]
pub async fn play_tracks(
    state: State<'_, AppState>,
    track_ids: Vec<u64>,
    start_index: usize,
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
pub async fn set_shuffle(state: State<'_, AppState>, enabled: bool) -> CmdResult<()> {
    send_cmd(&state, Cmd::SetShuffle(enabled)).await
}

#[tauri::command]
pub async fn set_repeat(state: State<'_, AppState>, mode: Repeat) -> CmdResult<()> {
    send_cmd(&state, Cmd::SetRepeat(mode)).await
}

#[tauri::command]
pub async fn queue_state(state: State<'_, AppState>) -> CmdResult<QueueView> {
    let (tx, rx) = oneshot::channel();
    send_cmd(&state, Cmd::GetQueue(tx)).await?;
    rx.await.map_err(|_| "lecteur arrêté".to_owned())
}

#[tauri::command]
pub async fn seek(state: State<'_, AppState>, seconds: f64) -> CmdResult<()> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err("position invalide".into());
    }
    send_cmd(&state, Cmd::Seek(seconds)).await
}

#[tauri::command]
pub async fn player_state(state: State<'_, AppState>) -> CmdResult<PlayerState> {
    let (tx, rx) = oneshot::channel();
    send_cmd(&state, Cmd::GetState(tx)).await?;
    rx.await.map_err(|_| "lecteur arrêté".to_owned())
}

async fn send_cmd(state: &AppState, cmd: Cmd) -> CmdResult<()> {
    state
        .player
        .send(cmd)
        .await
        .map_err(|_| "lecteur arrêté".to_owned())
}
