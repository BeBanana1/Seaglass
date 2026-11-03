//! Plein écran de la fenêtre principale (#131), bureau seulement : Android est déjà en
//! plein écran. L'UI ne fait que demander et afficher l'état renvoyé ici.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter, Manager, WebviewWindow, Window};

use super::{CmdResult, err};
use crate::i18n::tr;

const MAIN: &str = "main";

/// Dernier état envoyé à l'UI, pour n'émettre `fullscreen` que s'il change.
static FULLSCREEN: AtomicBool = AtomicBool::new(false);

fn main_window(app: &AppHandle) -> CmdResult<WebviewWindow> {
    app.get_webview_window(MAIN)
        .ok_or_else(|| tr("Fenêtre principale introuvable", "Main window not found").to_string())
}

#[tauri::command]
pub async fn fullscreen(app: AppHandle) -> CmdResult<bool> {
    main_window(&app)?.is_fullscreen().map_err(err)
}

/// Passe la fenêtre en plein écran ou l'en sort, puis renvoie l'état relu.
#[tauri::command]
pub async fn set_fullscreen(app: AppHandle, fullscreen: bool) -> CmdResult<bool> {
    let window = main_window(&app)?;
    window.set_fullscreen(fullscreen).map_err(err)?;
    let state = window.is_fullscreen().map_err(err)?;
    notify(&app, state);
    Ok(state)
}

/// Appelé sur chaque redimensionnement : l'état peut aussi changer hors de l'app.
pub(crate) fn on_resized(window: &Window) {
    if window.label() != MAIN {
        return;
    }
    if let Ok(state) = window.is_fullscreen() {
        notify(window.app_handle(), state);
    }
}

fn notify(app: &AppHandle, state: bool) {
    if FULLSCREEN.swap(state, Ordering::Relaxed) != state {
        let _ = app.emit("fullscreen", state);
    }
}
