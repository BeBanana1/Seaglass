//! Raccourcis clavier globaux (#413, bureau) : enregistrés auprès du système par
//! `tauri-plugin-global-shortcut`, pilotés seulement d'ici (aucune permission côté UI).
//!
//! Le plugin est ajouté pendant `setup` par `init`, et non dans le constructeur de l'app :
//! sous Linux sans serveur X, sa création échoue, et un plugin du constructeur empêcherait
//! alors l'app de démarrer. Les actions du lecteur partent directement à l'acteur ; « Aimer »
//! est annoncé à l'UI (`global-shortcut`), qui l'exécute par son registre d'actions.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

use super::tray;
use crate::commands::AppState;
use crate::commands::settings::global_shortcuts::{GlobalAction, GlobalShortcuts};
use crate::player::Cmd;

/// Cran de volume, le même que Ctrl+↑ / Ctrl+↓ dans l'app.
const VOLUME_STEP: i8 = 5;

/// Combinaisons enregistrées (identifiant de `global-hotkey`) et leur action.
static BOUND: Mutex<Vec<(u32, GlobalAction)>> = Mutex::new(Vec::new());
/// Actions dont la combinaison n'a pas pu être enregistrée (prise par une autre app, touche
/// inconnue), pour les Réglages.
static FAILED: Mutex<Vec<GlobalAction>> = Mutex::new(Vec::new());

/// Ajoute le plugin ; `false` si le système n'offre pas de raccourcis globaux.
pub fn init(app: &AppHandle) -> bool {
    let plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(on_shortcut)
        .build();
    match app.plugin(plugin) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!("raccourcis globaux indisponibles : {e}");
            false
        }
    }
}

/// Raccourcis globaux possibles ici (plugin chargé).
pub fn available(app: &AppHandle) -> bool {
    app.try_state::<tauri_plugin_global_shortcut::GlobalShortcut<Wry>>()
        .is_some()
}

/// Actions dont la combinaison n'est pas enregistrée.
pub fn failed() -> Vec<GlobalAction> {
    FAILED.lock().map(|f| f.clone()).unwrap_or_default()
}

/// Remplace les raccourcis enregistrés par ceux du réglage ; renvoie les actions dont la
/// combinaison n'a pas pu être prise.
pub fn apply(app: &AppHandle, settings: &GlobalShortcuts) -> Vec<GlobalAction> {
    let mut bound = Vec::new();
    let mut failed = Vec::new();
    if available(app) {
        let manager = app.global_shortcut();
        if let Err(e) = manager.unregister_all() {
            tracing::warn!("raccourcis globaux : retrait impossible : {e}");
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
                        tracing::info!("raccourci global {keys} non enregistré : {e}");
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
                tracing::warn!("raccourci global : UI injoignable : {e}");
            }
            return;
        }
    };
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if state.player.try_send(cmd).is_err() {
        tracing::warn!("raccourci global : lecteur indisponible");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les combinaisons proposées et celles que construit l'UI (`src/lib/hotkeys.ts`) sont
    /// lisibles par `global-hotkey`.
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
