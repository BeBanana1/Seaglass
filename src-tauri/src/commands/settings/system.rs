//! What the system allows: platform, launch at startup (the registry's `Run` key), Discord
//! presence and exclusive mode.

use serde::Serialize;
use tauri::AppHandle;
#[cfg(desktop)]
use tauri_plugin_autostart::ManagerExt;

use super::super::CmdResult;
#[cfg(desktop)]
use super::super::err;

/// The app's platform, so the UI picks its layout (touch on Android) and hides settings
/// that do not apply. Decided at compile time.
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

/// Actual state of the launch at startup, read from the system on every call: it may have
/// been changed elsewhere (Task Manager), so nothing is kept in the database.
/// Always false on Android, which has no launch at startup.
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

/// True if the Discord presence can be offered (#215): desktop, Discord application id
/// built in or given at launch.
#[tauri::command]
pub async fn discord_available() -> CmdResult<bool> {
    #[cfg(desktop)]
    return Ok(crate::integrations::discord::client_id().is_some());
    #[cfg(mobile)]
    Ok(false)
}

/// True if exclusive mode can be offered (#245): mpv on Windows only (#17).
#[tauri::command]
pub async fn exclusive_available() -> CmdResult<bool> {
    Ok(crate::player::exclusive_supported())
}

/// Turns the launch at startup on or off, then returns the state read back.
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

/// A `Run` entry created by a version older than #83 lacks `--minimized`: rewritten at
/// launch if it is on (turned off in the Task Manager, it is not touched).
#[cfg(desktop)]
pub(crate) fn refresh_autostart(app: &AppHandle) {
    let manager = app.autolaunch();
    if manager.is_enabled().unwrap_or(false)
        && let Err(e) = manager.enable()
    {
        tracing::warn!("launch at startup not updated: {e}");
    }
}
