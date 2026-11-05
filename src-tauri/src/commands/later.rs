//! « Plus tard » (Save for later, #196) : la liste de TIDAL où l'on met de côté titres,
//! albums, artistes et playlists.

use tauri::{AppHandle, Emitter, State};

use super::catalog::is_uuid;
use super::{AppState, CmdResult, err};
use crate::i18n::tr;
use crate::tidal::models::{LaterItem, LibraryList, LikeTarget, LikedIds};

/// Événement émis à chaque changement de la liste, avec tous les identifiants.
pub const LATER_EVENT: &str = "later";

/// Identifiants des éléments de la liste (les mixes y sont toujours vides).
#[tauri::command]
pub async fn later_ids(state: State<'_, AppState>) -> CmdResult<LikedIds> {
    state.tidal.later_ids().await.map_err(err)
}

/// Toute la liste, du plus récent au plus ancien (onglet de la bibliothèque).
#[tauri::command]
pub async fn save_for_later(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<LibraryList<LaterItem>> {
    let list = state.tidal.save_for_later().await.map_err(err)?;
    // Lecture complète : elle fait foi pour les menus (ajouts faits sur un autre appareil).
    emit_later(&app, &state).await;
    Ok(list)
}

/// Ajoute à la liste ou retire (titre, album, artiste, playlist).
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
                tracing::warn!("émission de {LATER_EVENT} impossible : {e}");
            }
        }
        Err(e) => tracing::warn!("« Plus tard » illisible : {e}"),
    }
}

/// L'identifiant finit dans le corps de la requête ; un mix n'y a pas sa place.
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
