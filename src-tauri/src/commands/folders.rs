//! Playlist folders (#331): reading, creating, renaming, filing, deleting.

use tauri::State;

use super::catalog::is_uuid;
use super::{AppState, CmdResult, err};
use crate::i18n::{tr, trf};
use crate::tidal::models::PlaylistFolder;

const MAX_NAME_LEN: usize = 200;
/// Playlists filed at once.
const MAX_MOVE: usize = 500;

/// The user's folders; empty list if they have none.
#[tauri::command]
pub async fn playlist_folders(state: State<'_, AppState>) -> CmdResult<Vec<PlaylistFolder>> {
    state.tidal.playlist_folders().await.map_err(err)
}

/// Creates a folder, with these playlists inside.
#[tauri::command]
pub async fn create_folder(
    state: State<'_, AppState>,
    name: String,
    uuids: Vec<String>,
) -> CmdResult<PlaylistFolder> {
    let name = clean_name(&name)?;
    validate_uuids(&uuids)?;
    state.tidal.create_folder(name, &uuids).await.map_err(err)
}

#[tauri::command]
pub async fn rename_folder(state: State<'_, AppState>, id: String, name: String) -> CmdResult<()> {
    validate_id(&id)?;
    let name = clean_name(&name)?;
    state.tidal.rename_folder(&id, name).await.map_err(err)
}

/// Files playlists in a folder, or takes them back out to the root (`folderId` absent).
#[tauri::command]
pub async fn move_to_folder(
    state: State<'_, AppState>,
    uuids: Vec<String>,
    folder_id: Option<String>,
) -> CmdResult<()> {
    validate_uuids(&uuids)?;
    if uuids.is_empty() {
        return Ok(());
    }
    if let Some(id) = &folder_id {
        validate_id(id)?;
    }
    state
        .tidal
        .move_to_folder(&uuids, folder_id.as_deref())
        .await
        .map_err(err)
}

/// Deletes a folder; its playlists are kept, at the root.
#[tauri::command]
pub async fn delete_folder(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    validate_id(&id)?;
    state.tidal.delete_folder(&id).await.map_err(err)
}

fn clean_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(tr("nom de dossier vide", "empty folder name").into());
    }
    if name.chars().count() > MAX_NAME_LEN {
        return Err(trf!(
            "nom limité à {MAX_NAME_LEN} caractères",
            "name limited to {MAX_NAME_LEN} characters"
        ));
    }
    Ok(name)
}

/// The identifier ends up in a TRN passed in the query: it is a uuid (seen on 2026-11-12).
fn validate_id(id: &str) -> Result<(), String> {
    if is_uuid(id) {
        Ok(())
    } else {
        Err("identifiant de dossier invalide".into())
    }
}

fn validate_uuids(uuids: &[String]) -> Result<(), String> {
    if uuids.len() > MAX_MOVE {
        return Err(format!("at most {MAX_MOVE} playlists"));
    }
    if uuids.iter().all(|u| is_uuid(u)) {
        Ok(())
    } else {
        Err("identifiant de playlist invalide".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "ff035142-e782-4731-899a-c815500743c5";

    #[test]
    fn name_validation() {
        assert_eq!(clean_name("  Jazz "), Ok("Jazz"));
        assert!(clean_name("  ").is_err());
        assert!(clean_name(&"é".repeat(MAX_NAME_LEN + 1)).is_err());
    }

    #[test]
    fn id_validation() {
        assert!(validate_id(UUID).is_ok());
        assert!(validate_id("root").is_err());
        assert!(validate_id("a,trn:folder:b").is_err());
        assert!(validate_uuids(&[UUID.to_owned()]).is_ok());
        assert!(validate_uuids(&["x,trn:playlist:y".to_owned()]).is_err());
        assert!(validate_uuids(&vec![UUID.to_owned(); MAX_MOVE + 1]).is_err());
    }
}
