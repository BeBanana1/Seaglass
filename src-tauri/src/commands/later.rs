//! "Later" (Save for later, #196): TIDAL's list where tracks,
//! albums, artists and playlists are set aside.

use tauri::{AppHandle, Emitter, State};

use super::catalog::is_uuid;
use super::{AppState, CmdResult, err};
use crate::i18n::tr;
use crate::tidal::models::{LaterItem, LibraryList, LikeTarget, LikedIds};

/// Event emitted on every change of the list, with all the identifiers.
pub const LATER_EVENT: &str = "later";

/// Identifiers of the list's items (mixes are always empty there).
#[tauri::command]
pub async fn later_ids(state: State<'_, AppState>) -> CmdResult<LikedIds> {
    state.tidal.later_ids().await.map_err(err)
}

/// The whole list, from most recent to oldest (library tab).
#[tauri::command]
pub async fn save_for_later(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<LibraryList<LaterItem>> {
    let list = state.tidal.save_for_later().await.map_err(err)?;
    // Full read: it is authoritative for the menus (additions made on another device).
    emit_later(&app, &state).await;
    Ok(list)
}

/// Adds to or removes from the list (track, album, artist, playlist).
#[tauri::command]
pub async fn set_saved_for_later(
    app: AppHandle,
    state: State<'_, AppState>,
    target: LikeTarget,
    saved: bool,
) -> CmdResult<()> {
    validate(&target)?;
    state
        .tidal
        .set_saved_for_later(&target, saved)
        .await
        .map_err(err)?;
    emit_later(&app, &state).await;
    Ok(())
}

async fn emit_later(app: &AppHandle, state: &AppState) {
    match state.tidal.later_ids().await {
        Ok(ids) => {
            if let Err(e) = app.emit(LATER_EVENT, ids) {
                tracing::warn!("could not emit {LATER_EVENT}: {e}");
            }
        }
        Err(e) => tracing::warn!("\"Later\" list unreadable: {e}"),
    }
}

/// The identifier ends up in the request body; a mix has no place there.
fn validate(target: &LikeTarget) -> Result<(), String> {
    match target {
        LikeTarget::Playlist { uuid } if !is_uuid(uuid) => {
            Err("identifiant de playlist invalide".into())
        }
        LikeTarget::Mix { .. } => Err(tr(
            "un mix ne peut pas être enregistré pour plus tard",
            "a mix cannot be saved for later",
        )
        .into()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_validation() {
        assert!(validate(&LikeTarget::Album { id: 1 }).is_ok());
        let playlist = |uuid: &str| LikeTarget::Playlist {
            uuid: uuid.to_owned(),
        };
        assert!(validate(&playlist("4ba30155-d807-446f-9e53-b67520ccfe83")).is_ok());
        assert!(validate(&playlist("../me")).is_err());
        let mix = LikeTarget::Mix {
            id: "0006ae53cfa4f377b530de077266e3".to_owned(),
        };
        assert!(validate(&mix).is_err());
    }
}
