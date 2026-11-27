//! Local listening history (#202): the "Played in Seaglass" section of the home page, most
//! played albums for "Featured", clearing. Plays are recorded by the actor
//! (`player/listen.rs`); nothing is sent to TIDAL.

use serde::Serialize;
use tauri::State;

use super::{AppState, CmdResult, err};
use crate::i18n::tr;
use crate::store::{Span, Store};
use crate::tidal::models::{AlbumSummary, HomeRow, MediaItem};

/// Key of the home section, stable like those of TIDAL's feed (customizable).
pub const HOME_KEY: &str = "SEAGLASS_RECENT";
const RECENT_LIMIT: usize = 20;
/// "Featured": albums played at least twice in the last 30 days.
const TOP_WINDOW_MS: i64 = 30 * 24 * 3600 * 1000;
const TOP_MIN_PLAYS: i64 = 2;
const TOP_LIMIT: usize = 10;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayHistory {
    /// Albums played, from most recent to oldest.
    pub recent: Vec<AlbumSummary>,
    /// Most played albums recently.
    pub top: Vec<AlbumSummary>,
}

pub(super) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

fn load(store: &Store) -> crate::store::Result<PlayHistory> {
    Ok(PlayHistory {
        recent: store.recent_albums(RECENT_LIMIT)?,
        top: store
            .top_albums(
                Span::since(now_ms() - TOP_WINDOW_MS),
                TOP_MIN_PLAYS,
                TOP_LIMIT,
            )?
            .into_iter()
            .map(|(album, _)| album)
            .collect(),
    })
}

#[tauri::command]
pub async fn play_history(state: State<'_, AppState>) -> CmdResult<PlayHistory> {
    let Some(store) = state.store.clone() else {
        return Ok(PlayHistory::default());
    };
    tauri::async_runtime::spawn_blocking(move || load(&store))
        .await
        .map_err(err)?
        .map_err(err)
}

#[tauri::command]
pub async fn clear_play_history(state: State<'_, AppState>) -> CmdResult<()> {
    let Some(store) = state.store.clone() else {
        return Ok(());
    };
    tauri::async_runtime::spawn_blocking(move || store.clear_plays())
        .await
        .map_err(err)?
        .map_err(err)
}

/// Home row of the played albums, at the top of the feed (before the layout), or nothing
/// if the history is empty.
pub(super) fn home_row(state: &AppState) -> Option<HomeRow> {
    let albums = state
        .store
        .as_deref()?
        .recent_albums(RECENT_LIMIT)
        .inspect_err(|e| tracing::warn!("listening history unreadable: {e}"))
        .ok()?;
    (!albums.is_empty()).then(|| HomeRow {
        key: HOME_KEY.to_owned(),
        title: tr("Écoutés dans Seaglass", "Played in Seaglass").to_owned(),
        items: albums.into_iter().map(MediaItem::Album).collect(),
    })
}

/// Sign-out: the previous person's history disappears.
pub(super) fn clear(state: &AppState) {
    if let Some(store) = &state.store
        && let Err(e) = store.clear_plays()
    {
        tracing::warn!("listening history not cleared: {e}");
    }
}
