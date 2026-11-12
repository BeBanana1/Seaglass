//! Import d'une playlist d'un autre service (#328) : aperçu des correspondances TIDAL. Rien
//! n'est écrit ici : « Créer une playlist » passe par `create_playlist` avec les titres que
//! l'utilisateur a gardés.
//!
//! Le filtre du contenu (#368) ne s'applique pas : l'utilisateur a choisi cette liste.

use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use super::{AppState, CmdResult};
use crate::foreign::import::{Matched, match_all};
use crate::foreign::playlists::{self, MAX_IMPORT, PlaylistRef};
use crate::i18n::tr;
use crate::tidal::models::Track;

/// Avancement envoyé à l'UI pendant la recherche des titres.
pub const PROGRESS_EVENT: &str = "import-progress";

/// Numéro de l'import en cours : un nouvel import (ou `cancel_import`) arrête le précédent.
static GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub name: String,
    /// `deezer`, `spotify` ou `apple`.
    pub service: &'static str,
    /// Titres de la playlist chez le service.
    pub total: usize,
    /// Titres lus : moins que `total` si le service n'en montre qu'une partie (Spotify : 100)
    /// ou au-delà de `MAX_IMPORT`.
    pub read: usize,
    pub items: Vec<ImportItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportItem {
    /// Titre et artiste chez le service d'origine.
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

/// Lit la playlist du lien (adresse rendue par `open_link`) et cherche chaque titre sur
/// TIDAL ; l'avancement part dans `import-progress`.
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
        // Une étape tous les 5 titres suffit à la barre.
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

/// Arrête l'import en cours (page quittée).
#[tauri::command]
pub async fn cancel_import() -> CmdResult<()> {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    Ok(())
}
