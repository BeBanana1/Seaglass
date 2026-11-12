//! Historique d'écoute local (#202) : section « Écoutés dans Seaglass » de l'accueil, albums
//! les plus écoutés pour « À la une », effacement. Les écoutes sont enregistrées par
//! l'acteur (`player/listen.rs`) ; rien n'est envoyé à TIDAL.

use serde::Serialize;
use tauri::State;

use super::{AppState, CmdResult, err};
use crate::i18n::tr;
use crate::store::Store;
use crate::tidal::models::{AlbumSummary, HomeRow, MediaItem};

/// Clé de la section d'accueil, stable comme celles du flux de TIDAL (personnalisable).
pub const HOME_KEY: &str = "SEAGLASS_RECENT";
const RECENT_LIMIT: usize = 20;
/// « À la une » : albums écoutés au moins deux fois ces 30 derniers jours.
const TOP_WINDOW_MS: i64 = 30 * 24 * 3600 * 1000;
const TOP_MIN_PLAYS: i64 = 2;
const TOP_LIMIT: usize = 10;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayHistory {
    /// Albums écoutés, du plus récent au plus ancien.
    pub recent: Vec<AlbumSummary>,
    /// Albums les plus écoutés récemment.
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
            .top_albums(now_ms() - TOP_WINDOW_MS, TOP_MIN_PLAYS, TOP_LIMIT)?
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

/// Ligne d'accueil des albums écoutés, en tête du flux (avant la disposition), ou rien si
/// l'historique est vide.
pub(super) fn home_row(state: &AppState) -> Option<HomeRow> {
    let albums = state
        .store
        .as_deref()?
        .recent_albums(RECENT_LIMIT)
        .inspect_err(|e| tracing::warn!("historique d'écoute illisible : {e}"))
        .ok()?;
    (!albums.is_empty()).then(|| HomeRow {
        key: HOME_KEY.to_owned(),
        title: tr("Écoutés dans Seaglass", "Played in Seaglass").to_owned(),
        items: albums.into_iter().map(MediaItem::Album).collect(),
    })
}

/// Déconnexion : l'historique de la personne précédente disparaît.
pub(super) fn clear(state: &AppState) {
    if let Some(store) = &state.store
        && let Err(e) = store.clear_plays()
    {
        tracing::warn!("historique d'écoute non effacé : {e}");
    }
}
