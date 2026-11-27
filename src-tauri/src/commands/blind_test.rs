//! Blind test (#434): commands validated then sent to the player actor, which holds the
//! game. The state goes out in the `blind-test` event.

use tauri::State;
use tokio::sync::oneshot;

use super::{AppState, CmdResult};
use crate::i18n::{tr, trf};
use crate::jam::JamState;
use crate::player::Cmd;
use crate::player::blind::{Answer, BlindCmd, BlindTestView, MAX_ROUNDS, Mode, Party};
use crate::tidal::models::TrackId;

/// Maximum number of tracks accepted to draw the rounds from (liked tracks included).
const MAX_POOL: usize = 10_000;
const MAX_TITLE: usize = 200;
const MAX_ANSWER: usize = 200;

/// Starts a game of `rounds` rounds drawn from `track_ids`; `title` is the name of the
/// list, displayed. A game in progress is replaced (the earlier queue stays the one from
/// before the first game). With `jam`, the guests of the current Jam play too (#441); the
/// host answers if they want to (`host_plays`).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn blind_test_start(
    state: State<'_, AppState>,
    jam_state: State<'_, JamState>,
    track_ids: Vec<u64>,
    mode: Mode,
    rounds: usize,
    title: String,
    jam: Option<bool>,
    host_plays: Option<bool>,
) -> CmdResult<()> {
    let jam = jam.unwrap_or(false);
    if jam && jam_state.status().is_none() {
        return Err(tr(
            "aucune Jam en cours : démarrez-la pour jouer avec vos invités",
            "no Jam running: start it to play with your guests",
        )
        .into());
    }
    let party = Party {
        jam,
        host_plays: host_plays.unwrap_or(true),
    };
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
            party,
            reply,
        },
    )
    .await?;
    rx.await.map_err(|_| stopped())?
}

/// Answer to the current round (typed, suggested or "skip").
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

/// Replays the round's excerpt from its start.
#[tauri::command]
pub async fn blind_test_replay(state: State<'_, AppState>) -> CmdResult<()> {
    send(&state, BlindCmd::Replay).await
}

/// Reveals the round without waiting for the missing answers (with several players, #441).
#[tauri::command]
pub async fn blind_test_reveal(state: State<'_, AppState>) -> CmdResult<()> {
    send(&state, BlindCmd::Reveal).await
}

/// Next round, or end of the game after the last one.
#[tauri::command]
pub async fn blind_test_next(state: State<'_, AppState>) -> CmdResult<()> {
    send(&state, BlindCmd::Next).await
}

/// Leaves the game: the earlier queue comes back, paused.
#[tauri::command]
pub async fn blind_test_quit(state: State<'_, AppState>) -> CmdResult<()> {
    send(&state, BlindCmd::Quit).await
}

/// Current game, `null` otherwise (page opening, UI reload).
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
