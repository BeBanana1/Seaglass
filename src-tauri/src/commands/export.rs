//! Export de la bibliothèque (#330), bureau seulement : lit les listes, écrit les fichiers
//! dans `Documents/Seaglass/export-<date UTC>/` et ouvre ce dossier. Pas de boîte
//! « Enregistrer sous » : elle demanderait `tauri-plugin-dialog` (voir `docs/interface.md`).
//!
//! Les playlists sont lues une par une, avec l'événement `library-export` (`done`/`total`)
//! après chacune ; `cancel_library_export` arrête la lecture avant la suivante.

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

/// Un export à la fois.
static RUNNING: AtomicBool = AtomicBool::new(false);
static CANCELLED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    done: usize,
    total: usize,
}

/// Résultat montré à l'utilisateur.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    /// Dossier écrit (déjà ouvert dans l'explorateur).
    pub dir: String,
    pub tracks: usize,
    pub albums: usize,
    pub artists: usize,
    pub playlists: usize,
    /// Titres des playlists illisibles (supprimées, indisponibles), laissées de côté.
    pub failed: Vec<String>,
    /// Une liste au moins est coupée au plafond de chargement.
    pub truncated: bool,
}

/// Remet `RUNNING` à faux quoi qu'il arrive.
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
                tracing::warn!("export : playlist {} illisible : {e}", summary.uuid);
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
        tracing::warn!("export : ouverture du dossier impossible : {e}");
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

/// Arrête l'export en cours avant la playlist suivante.
#[tauri::command]
pub fn cancel_library_export() {
    CANCELLED.store(true, Ordering::SeqCst);
}

fn emit(app: &AppHandle, done: usize, total: usize) {
    if let Err(e) = app.emit(PROGRESS_EVENT, Progress { done, total }) {
        tracing::warn!("événement {PROGRESS_EVENT} non émis : {e}");
    }
}

fn write_all(dir: &Path, files: &[export::File]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for file in files {
        std::fs::write(dir.join(&file.name), &file.content)?;
    }
    Ok(())
}
