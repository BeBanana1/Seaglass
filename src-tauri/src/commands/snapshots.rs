//! Copies des listes gardées d'un lancement à l'autre (#181) : pages de playlist et onglets
//! de la bibliothèque. L'UI affiche la copie aussitôt, puis la version à jour : une
//! playlist est revalidée par son ETag (#152), un onglet de la bibliothèque toujours relu
//! en entier (l'API ne donne pas de révision).
//!
//! Écritures et lectures sur un thread bloquant (`spawn_blocking`) : la base est partagée
//! avec la sauvegarde de la file. Les écritures partent sans être attendues ; deux écritures
//! de la même clé peuvent donc s'inverser, ce qui ne laisse au pire qu'une copie un peu
//! ancienne, remplacée à l'ouverture suivante.

use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tauri::State;

use super::catalog::check_playlist_uuid;
use super::{AppState, CmdResult};
use crate::store::{SnapshotLimit, Store};
use crate::tidal::models::{
    AlbumSummary, ArtistSummary, LibraryList, PlaylistPage, PlaylistSummary, TrackEntry,
};

/// Comme en mémoire (`tidal/playlist.rs`) : 20 playlists, 20 000 titres au plus.
const PLAYLISTS: SnapshotLimit = SnapshotLimit {
    prefix: "playlist:",
    count: 20,
    size: 20_000,
};

/// Onglets de la bibliothèque gardés. Les mixes n'en sont pas : leurs images sont des URLs
/// signées qui expirent, et leur contenu change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LibraryTab {
    Tracks,
    Albums,
    Artists,
    Playlists,
}

impl LibraryTab {
    fn key(self) -> &'static str {
        match self {
            Self::Tracks => "library:tracks",
            Self::Albums => "library:albums",
            Self::Artists => "library:artists",
            Self::Playlists => "library:playlists",
        }
    }
}

/// Copie d'un onglet, sérialisée comme la réponse de la commande de l'onglet.
#[derive(Serialize)]
#[serde(untagged)]
pub enum CachedLibrary {
    Tracks(LibraryList<TrackEntry>),
    Albums(LibraryList<AlbumSummary>),
    Artists(LibraryList<ArtistSummary>),
    Playlists(LibraryList<PlaylistSummary>),
}

fn playlist_key(uuid: &str) -> String {
    format!("{}{uuid}", PLAYLISTS.prefix)
}

/// Copie de la playlist : en mémoire, sinon sur le disque (gardée ensuite en mémoire).
#[tauri::command]
pub async fn cached_playlist(
    state: State<'_, AppState>,
    uuid: String,
) -> CmdResult<Option<PlaylistPage>> {
    check_playlist_uuid(&uuid)?;
    if let Some(page) = state.tidal.cached_playlist(&uuid) {
        return Ok(Some(page));
    }
    let page: Option<PlaylistPage> = load(state.store.clone(), playlist_key(&uuid)).await;
    if let Some(page) = &page {
        state.tidal.restore_playlist(&uuid, page.clone());
    }
    Ok(page)
}

/// Copie de l'onglet enregistrée au dernier chargement complet, `None` sans copie.
#[tauri::command]
pub async fn cached_library(
    state: State<'_, AppState>,
    tab: LibraryTab,
) -> CmdResult<Option<CachedLibrary>> {
    let store = state.store.clone();
    let key = tab.key().to_owned();
    Ok(match tab {
        LibraryTab::Tracks => load::<LibraryList<TrackEntry>>(store, key)
            .await
            .map(|list| {
                // La lecture peut partir de la copie avant la relecture : titres dans le cache.
                state
                    .tidal
                    .cache
                    .insert_all(list.items.iter().map(|e| &e.track));
                CachedLibrary::Tracks(list)
            }),
        LibraryTab::Albums => load(store, key).await.map(CachedLibrary::Albums),
        LibraryTab::Artists => load(store, key).await.map(CachedLibrary::Artists),
        LibraryTab::Playlists => load(store, key).await.map(CachedLibrary::Playlists),
    })
}

/// Page lue en entier : gardée si elle a une révision (sans, rien ne dirait si elle est
/// encore juste).
pub(super) fn save_playlist(state: &AppState, page: &PlaylistPage) {
    if page.revision.is_none() {
        return;
    }
    let key = playlist_key(&page.playlist.uuid);
    save(state, key, page.clone(), page.tracks.len(), Some(PLAYLISTS));
}

/// Playlist modifiée par l'app : sa copie ne vaut plus rien.
pub(super) fn forget_playlist(state: &AppState, uuid: &str) {
    let Some(store) = state.store.clone() else {
        return;
    };
    let key = playlist_key(uuid);
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = store.remove_snapshot(&key) {
            tracing::warn!("copie {key} non effacée : {e}");
        }
    });
}

pub(super) fn save_library<T: Serialize + Clone + Send + 'static>(
    state: &AppState,
    tab: LibraryTab,
    list: &LibraryList<T>,
) {
    save(
        state,
        tab.key().to_owned(),
        list.clone(),
        list.items.len(),
        None,
    );
}

/// Connexion ou déconnexion : les copies décrivent la bibliothèque d'un autre compte.
/// Attendu, pour qu'aucune copie ne soit relue après.
pub(super) async fn clear(state: &AppState) {
    let Some(store) = state.store.clone() else {
        return;
    };
    let cleared = tauri::async_runtime::spawn_blocking(move || store.clear_snapshots()).await;
    match cleared {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::warn!("copies des listes non effacées : {e}"),
        Err(e) => tracing::warn!("copies des listes non effacées : {e}"),
    }
}

async fn load<T: DeserializeOwned + Send + 'static>(
    store: Option<Arc<Store>>,
    key: String,
) -> Option<T> {
    let store = store?;
    let read = tauri::async_runtime::spawn_blocking(move || {
        let read = store.get_snapshot::<T>(&key);
        (key, read)
    })
    .await;
    match read {
        Ok((_, Ok(value))) => value,
        Ok((key, Err(e))) => {
            tracing::warn!("copie {key} illisible : {e}");
            None
        }
        Err(e) => {
            tracing::warn!("lecture d'une copie interrompue : {e}");
            None
        }
    }
}

fn save<T: Serialize + Send + 'static>(
    state: &AppState,
    key: String,
    value: T,
    size: usize,
    limit: Option<SnapshotLimit>,
) {
    let Some(store) = state.store.clone() else {
        return;
    };
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = store.put_snapshot(&key, &value, size, limit) {
            tracing::warn!("copie {key} non enregistrée : {e}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_without_mixes() {
        let tab: LibraryTab = serde_json::from_str("\"tracks\"").unwrap();
        assert_eq!(tab, LibraryTab::Tracks);
        assert!(serde_json::from_str::<LibraryTab>("\"mixes\"").is_err());
    }

    #[test]
    fn cached_library_has_the_shape_of_the_tab() {
        let list = LibraryList {
            items: vec![ArtistSummary {
                id: 1,
                name: "a".into(),
                picture_url: None,
            }],
            truncated: false,
            total: Some(1),
        };
        let json = serde_json::to_value(CachedLibrary::Artists(list.clone())).unwrap();
        assert_eq!(json, serde_json::to_value(&list).unwrap());
    }
}
