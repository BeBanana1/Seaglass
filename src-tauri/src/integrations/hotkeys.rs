//! Global keyboard shortcuts (#413, desktop): registered with the system by
//! `tauri-plugin-global-shortcut`, driven from here only (no permission on the UI side).
//!
//! The plugin is added during `setup` by `init`, not in the app builder: on Linux without an X
//! server, creating it fails, and a plugin of the builder would then keep the app from starting.
//! The player's actions go straight to the actor; "Like" is announced to the UI
//! (`global-shortcut`), which runs it through its action registry.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

use super::tray;
use crate::commands::AppState;
use crate::commands::settings::global_shortcuts::{GlobalAction, GlobalShortcuts};
use crate::player::Cmd;

/// Volume step, the same as Ctrl+↑ / Ctrl+↓ in the app.
const VOLUME_STEP: i8 = 5;

/// Registered combinations (`global-hotkey` identifier) and their action.
static BOUND: Mutex<Vec<(u32, GlobalAction)>> = Mutex::new(Vec::new());
/// Actions whose combination could not be registered (taken by another app, unknown key), for
/// Settings.
static FAILED: Mutex<Vec<GlobalAction>> = Mutex::new(Vec::new());

/// Adds the plugin; `false` if the system offers no global shortcuts.
pub fn init(app: &AppHandle) -> bool {
    let plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(on_shortcut)
        .build();
    match app.plugin(plugin) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!("global shortcuts unavailable: {e}");
            false
        }
    }
}

/// Whether global shortcuts are possible here (plugin loaded).
pub fn available(app: &AppHandle) -> bool {
    app.try_state::<tauri_plugin_global_shortcut::GlobalShortcut<Wry>>()
        .is_some()
}

/// Actions whose combination is not registered.
pub fn failed() -> Vec<GlobalAction> {
    FAILED.lock().map(|f| f.clone()).unwrap_or_default()
}

/// Replaces the registered shortcuts with those of the setting; returns the actions whose
/// combination could not be taken.
pub fn apply(app: &AppHandle, settings: &GlobalShortcuts) -> Vec<GlobalAction> {
    let mut bound = Vec::new();
    let mut failed = Vec::new();
    if available(app) {
        let manager = app.global_shortcut();
        if let Err(e) = manager.unregister_all() {
            tracing::warn!("global shortcuts: could not unregister: {e}");
        }
        if settings.enabled {
            for (&action, keys) in &settings.bindings {
                let registered = keys
                    .parse::<Shortcut>()
                    .map_err(|e| e.to_string())
                    .and_then(|shortcut| {
                        manager
                            .register(shortcut)
                            .map(|()| shortcut)
                            .map_err(|e| e.to_string())
                    });
                match registered {
                    Ok(shortcut) => bound.push((shortcut.id(), action)),
                    Err(e) => {
                        tracing::info!("global shortcut {keys} not registered: {e}");
                        failed.push(action);
                    }
                }
            }
        }
    } else if settings.enabled {
        failed.extend(settings.bindings.keys().copied());
    }
    if let Ok(mut b) = BOUND.lock() {
        *b = bound;
    }
    if let Ok(mut f) = FAILED.lock() {
        f.clone_from(&failed);
    }
    failed
}

fn on_shortcut(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state != ShortcutState::Pressed {
        return;
    }
    let action = BOUND.lock().ok().and_then(|b| {
        b.iter()
            .find(|(id, _)| *id == shortcut.id())
            .map(|(_, a)| *a)
    });
    let Some(action) = action else {
        return;
    };
    let cmd = match action {
        GlobalAction::PlayPause => Cmd::TogglePause,
        GlobalAction::Next => Cmd::Next,
        GlobalAction::Previous => Cmd::Previous,
        GlobalAction::VolumeUp => Cmd::StepVolume(VOLUME_STEP),
        GlobalAction::VolumeDown => Cmd::StepVolume(-VOLUME_STEP),
        GlobalAction::ShowWindow => return tray::show_main(app),
        GlobalAction::Like => {
            if let Err(e) = app.emit("global-shortcut", action) {
                tracing::warn!("global shortcut: UI unreachable: {e}");
            }
            return;
        }
    };
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if state.player.try_send(cmd).is_err() {
        tracing::warn!("global shortcut: player unavailable");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The offered combinations and those the UI builds (`src/lib/hotkeys.ts`) can be parsed by
    /// `global-hotkey`.
    #[test]
    fn combinations_from_the_ui_parse() {
        let defaults = GlobalShortcuts::default();
        let from_ui = [
            "Ctrl+Alt+Shift+KeyL",
            "Super+Digit5",
            "Ctrl+Alt+NumpadAdd",
            "Alt+F13",
            "Ctrl+Alt+MediaTrackPrevious",
            "Ctrl+Alt+AudioVolumeUp",
            "Ctrl+Shift+BracketLeft",
            "Ctrl+Alt+PageDown",
        ];
        for keys in defaults
            .bindings
            .values()
            .map(String::as_str)
            .chain(from_ui)
        {
            assert!(keys.parse::<Shortcut>().is_ok(), "{keys}");
        }
    }
}
