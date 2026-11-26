//! Ce que le système permet : plateforme, lancement au démarrage (clé `Run` du registre),
//! présence Discord et mode exclusif.

use serde::Serialize;
use tauri::AppHandle;
#[cfg(desktop)]
use tauri_plugin_autostart::ManagerExt;

use super::super::CmdResult;
#[cfg(desktop)]
use super::super::err;

/// Plateforme de l'app, pour que l'UI choisisse sa mise en page (tactile sur Android) et
/// masque les réglages sans objet. Décidée à la compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    #[cfg(desktop)]
    Desktop,
    #[cfg(mobile)]
    Android,
}

#[tauri::command]
pub async fn platform() -> CmdResult<Platform> {
    #[cfg(desktop)]
    return Ok(Platform::Desktop);
    #[cfg(mobile)]
    Ok(Platform::Android)
}

/// État réel du lancement au démarrage, lu dans le système à chaque appel : il peut avoir
/// été changé ailleurs (Gestionnaire des tâches), donc rien n'est gardé en base.
/// Toujours faux sur Android, qui n'a pas de lancement au démarrage.
#[tauri::command]
pub async fn autostart_enabled(app: AppHandle) -> CmdResult<bool> {
    #[cfg(desktop)]
    return app.autolaunch().is_enabled().map_err(err);
    #[cfg(mobile)]
    {
        let _ = app;
        Ok(false)
    }
}

/// Vrai si la présence Discord peut être proposée (#215) : bureau, identifiant de
/// l'application Discord compilé ou donné au lancement.
#[tauri::command]
pub async fn discord_available() -> CmdResult<bool> {
    #[cfg(desktop)]
    return Ok(crate::integrations::discord::client_id().is_some());
    #[cfg(mobile)]
    Ok(false)
}

/// Vrai si le mode exclusif peut être proposé (#245) : mpv sous Windows seulement (#17).
#[tauri::command]
pub async fn exclusive_available() -> CmdResult<bool> {
    Ok(crate::player::exclusive_supported())
}

/// Active ou désactive le lancement au démarrage, puis renvoie l'état relu.
#[cfg(desktop)]
#[tauri::command]
pub async fn set_autostart(app: AppHandle, enabled: bool) -> CmdResult<bool> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(err)?;
    } else {
        manager.disable().map_err(err)?;
    }
    manager.is_enabled().map_err(err)
}

/// Une entrée `Run` créée par une version antérieure à #83 n'a pas `--minimized` : réécrite au lancement si
/// elle est active (désactivée dans le Gestionnaire des tâches, elle n'est pas touchée).
#[cfg(desktop)]
pub(crate) fn refresh_autostart(app: &AppHandle) {
    let manager = app.autolaunch();
    if manager.is_enabled().unwrap_or(false)
        && let Err(e) = manager.enable()
    {
        tracing::warn!("lancement au démarrage non mis à jour : {e}");
    }
}
