//! Library export (#330), desktop only: reads the lists, writes the files
//! in `Documents/Seaglass/export-<UTC date>/` and opens that folder. No "Save as"
//! dialog: it would need `tauri-plugin-dialog` (see `docs/interface.md`).
//!
//! Playlists are read one by one, with the `library-export` event (`done`/`total`)
//! after each one; `cancel_library_export` stops reading before the next one.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use super::{AppState, CmdResult, err};
use crate::export::{self, Library, PlaylistExport};
use crate::i18n::{tr, trf};
use crate::utc::Utc;

const PROGRESS_EVENT: &str = "library-export";

/// One export at a time.
static RUNNING: AtomicBool = AtomicBool::new(false);
static CANCELLED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    done: usize,
    total: usize,
}

/// Result shown to the user.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    /// Folder written (already open in the file explorer).
    pub dir: String,
    pub tracks: usize,
    pub albums: usize,
    pub artists: usize,
    pub playlists: usize,
    /// Titles of unreadable playlists (deleted, unavailable), left aside.
    pub failed: Vec<String>,
    /// At least one list is cut at the loading cap.
    pub truncated: bool,
}

/// Resets `RUNNING` to false whatever happens.
struct RunGuard;

impl Drop for RunGuard {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::SeqCst);
    }
}

#[tauri::command]
pub async fn export_library(app: AppHandle, state: State<'_, AppState>) -> CmdResult<ExportReport> {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err(tr(
            "Un export est déjà en cours",
            "An export is already running",
        )
        .into());
    }
    let _guard = RunGuard;
    CANCELLED.store(false, Ordering::SeqCst);

    let tidal = &state.tidal;
    let (tracks, albums, artists, playlists) = tokio::try_join!(
        tidal.favorite_tracks(),
        tidal.favorite_albums(),
        tidal.favorite_artists(),
        tidal.library_playlists(),
    )
    .map_err(err)?;

    let total = playlists.items.len();
    let mut exported = Vec::with_capacity(total);
    let mut failed = Vec::new();
    emit(&app, 0, total);
    for (i, summary) in playlists.items.into_iter().enumerate() {
        if CANCELLED.load(Ordering::SeqCst) {
            return Err(tr("Export annulé", "Export cancelled").into());
        }
        match tidal.playlist_tracks(&summary.uuid).await {
            Ok(tracks) => exported.push(PlaylistExport { summary, tracks }),
            Err(e) => {
                tracing::warn!("export: playlist {} unreadable: {e}", summary.uuid);
                failed.push(summary.title);
            }
        }
        emit(&app, i + 1, total);
    }

    let library = Library {
        tracks,
        albums,
        artists,
        playlists: exported,
    };
    let report_counts = (
        library.tracks.items.len(),
        library.albums.items.len(),
        library.artists.items.len(),
        library.playlists.len(),
        library.tracks.truncated
            || library.albums.truncated
            || library.artists.truncated
            || playlists.truncated,
    );

    let now = Utc::now();
    let documents = app.path().document_dir().map_err(err)?;
    let dir = documents.join("Seaglass").join(format!(
        "export-{}-{:02}{:02}{:02}",
        now.date(),
        now.hour,
        now.minute,
        now.second
    ));
    let files = export::files(&library, now).map_err(err)?;
    let target = dir.clone();
    tokio::task::spawn_blocking(move || write_all(&target, &files))
        .await
        .map_err(err)?
        .map_err(|e| {
            trf!(
                "Écriture de l'export impossible : {e}",
                "Could not write the export: {e}"
            )
        })?;

    if let Err(e) = app.opener().open_path(dir.to_string_lossy(), None::<&str>) {
        tracing::warn!("export: could not open the folder: {e}");
    }
    let (tracks, albums, artists, playlists, truncated) = report_counts;
    Ok(ExportReport {
        dir: dir.to_string_lossy().into_owned(),
        tracks,
        albums,
        artists,
        playlists,
        failed,
        truncated,
    })
}

/// Stops the running export before the next playlist.
#[tauri::command]
pub fn cancel_library_export() {
    CANCELLED.store(true, Ordering::SeqCst);
}

fn emit(app: &AppHandle, done: usize, total: usize) {
    if let Err(e) = app.emit(PROGRESS_EVENT, Progress { done, total }) {
        tracing::warn!("event {PROGRESS_EVENT} not emitted: {e}");
    }
}

fn write_all(dir: &Path, files: &[export::File]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for file in files {
        std::fs::write(dir.join(&file.name), &file.content)?;
    }
    Ok(())
}
