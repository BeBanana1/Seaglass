//! Library write commands: favorites (#4, #81) and the user's playlists.

use tauri::{AppHandle, Emitter, State};

use super::catalog::{is_mix_id, is_uuid};
use super::{AppState, CmdResult, err};
use crate::i18n::{tr, trf};
use crate::tidal::models::{LikeTarget, LikedIds, PlaylistSummary, TrackId};

const MAX_TITLE_LEN: usize = 200;
const MAX_DESCRIPTION_LEN: usize = 2_000;
/// Tracks added at once (an album, a displayed playlist).
pub(crate) const MAX_ADD: usize = 10_000;

/// Event emitted on every change of the favorites, with all the lists.
pub const LIKES_EVENT: &str = "likes";

/// Identifiers of all the favorites: tracks, albums, artists, followed playlists.
#[tauri::command]
pub async fn likes(state: State<'_, AppState>) -> CmdResult<LikedIds> {
    state.tidal.liked_ids().await.map_err(err)
}

/// Adds to or removes from the favorites (track, album, artist, another user's playlist, mix).
#[tauri::command]
pub async fn set_liked(
    app: AppHandle,
    state: State<'_, AppState>,
    target: LikeTarget,
    liked: bool,
) -> CmdResult<()> {
    validate_target(&target)?;
    state.tidal.set_liked(&target, liked).await.map_err(err)?;
    emit_likes(&app, &state).await;
    Ok(())
}

/// Adds or removes several tracks from the favorites (multiple selection, #332).
#[tauri::command]
pub async fn set_tracks_liked(
    app: AppHandle,
    state: State<'_, AppState>,
    track_ids: Vec<TrackId>,
    liked: bool,
) -> CmdResult<()> {
    if track_ids.is_empty() || track_ids.len() > MAX_ADD {
        return Err(format!("1 to {MAX_ADD} tracks"));
    }
    state
        .tidal
        .set_tracks_liked(&track_ids, liked)
        .await
        .map_err(err)?;
    emit_likes(&app, &state).await;
    Ok(())
}

/// A position in a playlist and the track expected at that position.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistItem {
    index: usize,
    track_id: TrackId,
}

fn playlist_items(uuid: &str, items: &[PlaylistItem]) -> Result<Vec<(usize, TrackId)>, String> {
    if !is_uuid(uuid) {
        return Err("identifiant de playlist invalide".into());
    }
    if items.is_empty() || items.len() > MAX_ADD {
        return Err(format!("1 to {MAX_ADD} tracks"));
    }
    Ok(items.iter().map(|i| (i.index, i.track_id)).collect())
}

/// Removes several tracks from a playlist (#332); everything is refused if one of them is no longer
/// at its position.
#[tauri::command]
pub async fn remove_many_from_playlist(
    state: State<'_, AppState>,
    uuid: String,
    items: Vec<PlaylistItem>,
) -> CmdResult<()> {
    let items = playlist_items(&uuid, &items)?;
    super::snapshots::forget_playlist(&state, &uuid);
    state
        .tidal
        .remove_many_from_playlist(&uuid, &items)
        .await
        .map_err(err)
}

/// Undoes a batch removal: each track returns to its position; returns the number restored.
#[tauri::command]
pub async fn restore_many_to_playlist(
    state: State<'_, AppState>,
    uuid: String,
    items: Vec<PlaylistItem>,
) -> CmdResult<usize> {
    let items = playlist_items(&uuid, &items)?;
    super::snapshots::forget_playlist(&state, &uuid);
    state
        .tidal
        .restore_many_to_playlist(&uuid, &items)
        .await
        .map_err(err)
}

/// The editable playlists (created by the user), for "Add to a playlist".
#[tauri::command]
pub async fn own_playlists(state: State<'_, AppState>) -> CmdResult<Vec<PlaylistSummary>> {
    state.tidal.own_playlists().await.map_err(err)
}

/// Adds the tracks at the end of the playlist; returns the number actually added
/// (tracks already present are ignored).
#[tauri::command]
pub async fn add_to_playlist(
    state: State<'_, AppState>,
    uuid: String,
    track_ids: Vec<TrackId>,
) -> CmdResult<usize> {
    validate_tracks(&uuid, &track_ids)?;
    super::snapshots::forget_playlist(&state, &uuid);
    state
        .tidal
        .add_to_playlist(&uuid, &track_ids)
        .await
        .map_err(err)
}

/// Creates a playlist with these tracks; returns its uuid.
#[tauri::command]
pub async fn create_playlist(
    state: State<'_, AppState>,
    title: String,
    track_ids: Vec<TrackId>,
) -> CmdResult<String> {
    let title = clean_title(&title)?;
    if track_ids.len() > MAX_ADD {
        return Err(format!("at most {MAX_ADD} tracks"));
    }
    let uuid = state.tidal.create_playlist(title, "").await.map_err(err)?;
    if !track_ids.is_empty() {
        state
            .tidal
            .add_to_playlist(&uuid, &track_ids)
            .await
            .map_err(|e| {
                trf!(
                    "playlist créée, mais ajout des titres impossible : {e}",
                    "playlist created, but the tracks could not be added: {e}"
                )
            })?;
    }
    Ok(uuid)
}

/// Removes the track `track_id` at position `index` (0 = first); refused if the playlist
/// changed since it was displayed.
#[tauri::command]
pub async fn remove_from_playlist(
    state: State<'_, AppState>,
    uuid: String,
    index: usize,
    track_id: TrackId,
) -> CmdResult<()> {
    if !is_uuid(&uuid) {
        return Err("identifiant de playlist invalide".into());
    }
    super::snapshots::forget_playlist(&state, &uuid);
    state
        .tidal
        .remove_from_playlist(&uuid, index, track_id)
        .await
        .map_err(err)
}

/// Puts the track `track_id` back at position `index` (0 = first): undo of a removal
/// (#349). `false` if TIDAL added nothing.
#[tauri::command]
pub async fn restore_to_playlist(
    state: State<'_, AppState>,
    uuid: String,
    index: usize,
    track_id: TrackId,
) -> CmdResult<bool> {
    if !is_uuid(&uuid) {
        return Err("identifiant de playlist invalide".into());
    }
    super::snapshots::forget_playlist(&state, &uuid);
    state
        .tidal
        .insert_in_playlist(&uuid, index, track_id)
        .await
        .map_err(err)
}

/// Moves the track `track_id` from position `from` to position `to` (0 = first, #201);
/// refused if the playlist changed since it was displayed.
#[tauri::command]
pub async fn move_in_playlist(
    state: State<'_, AppState>,
    uuid: String,
    from: usize,
    to: usize,
    track_id: TrackId,
) -> CmdResult<()> {
    if !is_uuid(&uuid) {
        return Err("identifiant de playlist invalide".into());
    }
    if from == to {
        return Ok(());
    }
    super::snapshots::forget_playlist(&state, &uuid);
    state
        .tidal
        .move_in_playlist(&uuid, from, to, track_id)
        .await
        .map_err(err)
}

/// Renames a playlist and changes its description (#201).
#[tauri::command]
pub async fn edit_playlist(
    state: State<'_, AppState>,
    uuid: String,
    title: String,
    description: String,
) -> CmdResult<()> {
    if !is_uuid(&uuid) {
        return Err("identifiant de playlist invalide".into());
    }
    let title = clean_title(&title)?;
    let description = description.trim();
    if description.chars().count() > MAX_DESCRIPTION_LEN {
        return Err(trf!(
            "description limitée à {MAX_DESCRIPTION_LEN} caractères",
            "description limited to {MAX_DESCRIPTION_LEN} characters"
        ));
    }
    super::snapshots::forget_playlist(&state, &uuid);
    state
        .tidal
        .edit_playlist(&uuid, title, description)
        .await
        .map_err(err)
}

/// Deletes one of the user's playlists (#201).
#[tauri::command]
pub async fn delete_playlist(state: State<'_, AppState>, uuid: String) -> CmdResult<()> {
    if !is_uuid(&uuid) {
        return Err("identifiant de playlist invalide".into());
    }
    super::snapshots::forget_playlist(&state, &uuid);
    state.tidal.delete_playlist(&uuid).await.map_err(err)
}

fn clean_title(title: &str) -> Result<&str, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err(tr("titre de playlist vide", "empty playlist title").into());
    }
    if title.chars().count() > MAX_TITLE_LEN {
        return Err(trf!(
            "titre limité à {MAX_TITLE_LEN} caractères",
            "title limited to {MAX_TITLE_LEN} characters"
        ));
    }
    Ok(title)
}

/// After a write or a full read of a library category, the displayed
/// favorites may have changed.
pub(super) async fn emit_likes(app: &AppHandle, state: &AppState) {
    match state.tidal.liked_ids().await {
        Ok(ids) => {
            if let Err(e) = app.emit(LIKES_EVENT, ids) {
                tracing::warn!("could not emit {LIKES_EVENT}: {e}");
            }
        }
        Err(e) => tracing::warn!("favorites unreadable: {e}"),
    }
}

/// The uuid ends up in the request path: it is checked.
fn validate_target(target: &LikeTarget) -> Result<(), String> {
    match target {
        LikeTarget::Playlist { uuid } if !is_uuid(uuid) => {
            Err("identifiant de playlist invalide".into())
        }
        LikeTarget::Mix { id } if !is_mix_id(id) => Err("identifiant de mix invalide".into()),
        _ => Ok(()),
    }
}

fn validate_tracks(uuid: &str, track_ids: &[TrackId]) -> Result<(), String> {
    if !is_uuid(uuid) {
        return Err("identifiant de playlist invalide".into());
    }
    if track_ids.is_empty() {
        return Err(tr("aucun titre à ajouter", "no tracks to add").into());
    }
    if track_ids.len() > MAX_ADD {
        return Err(format!("at most {MAX_ADD} tracks"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "4ba30155-d807-446f-9e53-b67520ccfe83";

    #[test]
    fn target_validation() {
        assert!(validate_target(&LikeTarget::Album { id: 1 }).is_ok());
        let playlist = |uuid: &str| LikeTarget::Playlist {
            uuid: uuid.to_owned(),
        };
        assert!(validate_target(&playlist(UUID)).is_ok());
        assert!(validate_target(&playlist("../../sessions")).is_err());
        let mix = |id: &str| LikeTarget::Mix { id: id.to_owned() };
        assert!(validate_target(&mix("0006ae53cfa4f377b530de077266e3")).is_ok());
        assert!(validate_target(&mix("../ids")).is_err());
        let target: LikeTarget = serde_json::from_str(r#"{"kind":"artist","id":12979}"#).unwrap();
        assert_eq!(target, LikeTarget::Artist { id: 12_979 });
    }

    #[test]
    fn title_validation() {
        assert_eq!(clean_title("  Road trip "), Ok("Road trip"));
        assert!(clean_title("   ").is_err());
        assert!(clean_title(&"é".repeat(MAX_TITLE_LEN)).is_ok());
        assert!(clean_title(&"é".repeat(MAX_TITLE_LEN + 1)).is_err());
    }

    #[test]
    fn track_validation() {
        assert!(validate_tracks(UUID, &[TrackId(1)]).is_ok());
        assert!(validate_tracks(UUID, &[]).is_err());
        assert!(validate_tracks("../users/1", &[TrackId(1)]).is_err());
        assert!(validate_tracks(UUID, &vec![TrackId(1); MAX_ADD + 1]).is_err());
    }
}
