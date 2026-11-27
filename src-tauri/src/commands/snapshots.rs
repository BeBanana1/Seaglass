//! Copies of the lists kept from one launch to the next (#181): playlist pages and library
//! tabs. The UI displays the copy at once, then the up to date version: a
//! playlist is revalidated by its ETag (#152), a library tab is always read again
//! in full (the API gives no revision).
//!
//! Writes and reads on a blocking thread (`spawn_blocking`): the database is shared
//! with the queue saving. Writes are not awaited; two writes
//! of the same key can therefore swap, which at worst leaves a slightly
//! old copy, replaced at the next opening.

use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tauri::State;

use super::catalog::check_playlist_uuid;
use super::{AppState, CmdResult};
use crate::content::filtered;
use crate::store::{SnapshotLimit, Store};
use crate::tidal::models::{
    AlbumSummary, ArtistSummary, LibraryList, PlaylistPage, PlaylistSummary, TrackEntry,
};

/// As in memory (`tidal/playlist.rs`): 20 playlists, 20,000 tracks at most.
const PLAYLISTS: SnapshotLimit = SnapshotLimit {
    prefix: "playlist:",
    count: 20,
    size: 20_000,
};

/// Library tabs kept. Mixes are not among them: their images are signed URLs
/// that expire, and their content changes.
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

/// Copy of a tab, serialized like the tab command's response.
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

/// Copy of the playlist: in memory, otherwise on disk (then kept in memory).
#[tauri::command]
pub async fn cached_playlist(
    state: State<'_, AppState>,
    uuid: String,
) -> CmdResult<Option<PlaylistPage>> {
    check_playlist_uuid(&uuid)?;
    if let Some(page) = state.tidal.cached_playlist(&uuid) {
        return Ok(Some(filtered(page)));
    }
    let page: Option<PlaylistPage> = load(state.store.clone(), playlist_key(&uuid)).await;
    if let Some(page) = &page {
        state.tidal.restore_playlist(&uuid, page.clone());
    }
    Ok(filtered(page))
}

/// Copy of the tab recorded at the last full load, `None` without a copy.
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
                // Playback may start from the copy before the re-read: tracks in the cache.
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

/// Page read in full: kept if it has a revision (without one, nothing would say whether it
/// is still right).
pub(super) fn save_playlist(state: &AppState, page: &PlaylistPage) {
    if page.revision.is_none() {
        return;
    }
    let key = playlist_key(&page.playlist.uuid);
    save(state, key, page.clone(), page.tracks.len(), Some(PLAYLISTS));
}

/// Playlist modified by the app: its copy is worth nothing anymore.
pub(super) fn forget_playlist(state: &AppState, uuid: &str) {
    let Some(store) = state.store.clone() else {
        return;
    };
    let key = playlist_key(uuid);
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = store.remove_snapshot(&key) {
            tracing::warn!("copy {key} not cleared: {e}");
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

/// Sign-in or sign-out: the copies describe another account's library.
/// Awaited, so that no copy is read afterwards.
pub(super) async fn clear(state: &AppState) {
    let Some(store) = state.store.clone() else {
        return;
    };
    let cleared = tauri::async_runtime::spawn_blocking(move || store.clear_snapshots()).await;
    match cleared {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::warn!("list copies not cleared: {e}"),
        Err(e) => tracing::warn!("list copies not cleared: {e}"),
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
            tracing::warn!("copy {key} unreadable: {e}");
            None
        }
        Err(e) => {
            tracing::warn!("reading a copy interrupted: {e}");
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
            tracing::warn!("copy {key} not stored: {e}");
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
