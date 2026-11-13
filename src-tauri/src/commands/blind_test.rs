//! Blind test (#434) : commandes validées puis envoyées à l'acteur du lecteur, qui tient la
//! partie. L'état part dans l'événement `blind-test`.

use tauri::State;
use tokio::sync::oneshot;

use super::{AppState, CmdResult};
use crate::i18n::{tr, trf};
use crate::player::Cmd;
use crate::player::blind::{Answer, BlindCmd, BlindTestView, MAX_ROUNDS, Mode};
use crate::tidal::models::TrackId;

/// Titres acceptés au plus pour tirer les manches (les titres likés compris).
const MAX_POOL: usize = 10_000;
const MAX_TITLE: usize = 200;
const MAX_ANSWER: usize = 200;

/// Lance une partie de `rounds` manches tirées dans `track_ids` ; `title` est le nom de la
/// liste, affiché. Une partie en cours est remplacée (la file d'avant reste celle d'avant
/// la première).
#[tauri::command]
pub async fn blind_test_start(
    state: State<'_, AppState>,
    track_ids: Vec<u64>,
    mode: Mode,
    rounds: usize,
    title: String,
) -> CmdResult<()> {
    if track_ids.is_empty() || track_ids.len() > MAX_POOL {
        return Err(trf!(
            "la liste doit contenir entre 1 et {MAX_POOL} titres",
            "the list must contain 1 to {MAX_POOL} tracks"
        ));
    }
    if !(1..=MAX_ROUNDS).contains(&rounds) {
        return Err(trf!(
            "une partie compte de 1 à {MAX_ROUNDS} manches",
            "a game has 1 to {MAX_ROUNDS} rounds"
        ));
    }
    let title: String = title.trim().chars().take(MAX_TITLE).collect();
    let (reply, rx) = oneshot::channel();
    send(
        &state,
        BlindCmd::Start {
            tracks: track_ids.into_iter().map(TrackId).collect(),
            mode,
            rounds,
            title,
            reply,
        },
    )
    .await?;
    rx.await.map_err(|_| stopped())?
}

/// Réponse à la manche en cours (saisie, proposition ou « passer »).
#[tauri::command]
pub async fn blind_test_answer(state: State<'_, AppState>, answer: Answer) -> CmdResult<()> {
    let answer = match answer {
        Answer::Text { text } => Answer::Text {
            text: text.trim().chars().take(MAX_ANSWER).collect(),
        },
        other => other,
    };
    send(&state, BlindCmd::Answer(answer)).await
}

/// Réécoute l'extrait de la manche depuis son début.
#[tauri::command]
pub async fn blind_test_replay(state: State<'_, AppState>) -> CmdResult<()> {
    send(&state, BlindCmd::Replay).await
}

/// Manche suivante, ou fin de partie après la dernière.
#[tauri::command]
pub async fn blind_test_next(state: State<'_, AppState>) -> CmdResult<()> {
    send(&state, BlindCmd::Next).await
}

/// Quitte la partie : la file d'avant revient, en pause.
#[tauri::command]
pub async fn blind_test_quit(state: State<'_, AppState>) -> CmdResult<()> {
    send(&state, BlindCmd::Quit).await
}

/// Partie en cours, `null` sinon (ouverture de la page, rechargement de l'UI).
#[tauri::command]
pub async fn blind_test_state(state: State<'_, AppState>) -> CmdResult<Option<BlindTestView>> {
    let (reply, rx) = oneshot::channel();
    send(&state, BlindCmd::Get(reply)).await?;
    rx.await.map_err(|_| stopped())
}

async fn send(state: &AppState, cmd: BlindCmd) -> CmdResult<()> {
    state
        .player
        .send(Cmd::BlindTest(cmd))
        .await
        .map_err(|_| stopped())
}

fn stopped() -> String {
    tr("lecteur arrêté", "player stopped").into()
}
