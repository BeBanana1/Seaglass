//! Import of a playlist from another service (#328): preview of the TIDAL matches. Nothing
//! is written here: "Create a playlist" goes through `create_playlist` with the tracks the
//! user kept.
//!
//! The content filter (#368) does not apply: the user chose this list.

use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use super::{AppState, CmdResult};
use crate::foreign::import::{Matched, match_all};
use crate::foreign::playlists::{self, MAX_IMPORT, PlaylistRef};
use crate::i18n::tr;
use crate::tidal::models::Track;

/// Progress sent to the UI while searching for the tracks.
pub const PROGRESS_EVENT: &str = "import-progress";

/// Number of the import in progress: a new import (or `cancel_import`) stops the previous one.
static GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub name: String,
    /// `deezer`, `spotify` or `apple`.
    pub service: &'static str,
    /// Tracks of the playlist at the service.
    pub total: usize,
    /// Tracks read: fewer than `total` if the service only shows part of them (Spotify: 100)
    /// or beyond `MAX_IMPORT`.
    pub read: usize,
    pub items: Vec<ImportItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportItem {
    /// Title and artist at the original service.
    pub title: String,
    pub artist: Option<String>,
    pub track: Option<Track>,
    pub sure: bool,
}

#[derive(Debug, Clone, Serialize)]
struct Progress {
    done: usize,
    total: usize,
}

impl From<Matched> for ImportItem {
    fn from(m: Matched) -> Self {
        Self {
            title: m.entry.wanted.title,
            artist: m.entry.wanted.artist,
            track: m.track,
            sure: m.sure,
        }
    }
}

/// Reads the playlist of the link (address returned by `open_link`) and searches each track on
/// TIDAL; progress goes out in `import-progress`.
#[tauri::command]
pub async fn import_playlist(
    app: AppHandle,
    state: State<'_, AppState>,
    link: String,
) -> CmdResult<ImportPreview> {
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let playlist = match crate::foreign::playlist_ref_offline(&link) {
        Some(PlaylistRef::YouTube) => return Err(playlists::youtube_unsupported()),
        Some(p) => p,
        None => {
            return Err(tr(
                "Lien de playlist non reconnu (Deezer, Spotify, Apple Music)",
                "Unrecognised playlist link (Deezer, Spotify, Apple Music)",
            )
            .into());
        }
    };
    let listing = playlists::fetch(&playlist).await?;
    let read = listing.entries.len();
    let progress = |done: usize| {
        // One step every 5 tracks is enough for the bar.
        if done.is_multiple_of(5) || done == read {
            let _ = app.emit(PROGRESS_EVENT, Progress { done, total: read });
        }
    };
    let cancelled = || GENERATION.load(Ordering::SeqCst) != generation;
    progress(0);
    let matched = match_all(&state.tidal, listing.entries, &progress, &cancelled).await?;
    Ok(ImportPreview {
        name: listing.name,
        service: playlist.service(),
        total: listing.total,
        read: read.min(MAX_IMPORT),
        items: matched.into_iter().map(Into::into).collect(),
    })
}

/// Stops the import in progress (page left).
#[tauri::command]
pub async fn cancel_import() -> CmdResult<()> {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    Ok(())
}
