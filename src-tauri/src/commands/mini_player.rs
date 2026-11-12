//! Mini-lecteur du bureau (#384) : petite fenêtre sans bordure, au premier plan au choix,
//! qui remplace la fenêtre principale tant qu'elle est ouverte. Elle charge la même UI, qui
//! se reconnaît à l'étiquette de sa fenêtre et ne fait qu'afficher `player-state` et envoyer
//! les commandes du lecteur. La position et « au premier plan » sont gardés dans les réglages
//! SQLite.
//!
//! Fermer le mini-lecteur (son bouton, Alt+F4) réaffiche la fenêtre principale ; réafficher
//! la fenêtre principale (zone de notification, relance de l'app) ferme le mini-lecteur
//! (`tray::show_main`). Les deux ne sont jamais visibles ensemble.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Monitor, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

use super::{AppState, CmdResult, err};
use crate::integrations::tray;

/// Étiquette de la fenêtre, reprise par `capabilities/mini.json` et l'UI.
pub const LABEL: &str = "mini";
const KEY: &str = "mini_player";
/// Taille logique de la fenêtre : l'UI du mini-lecteur est dessinée pour elle.
const WIDTH: f64 = 360.0;
const HEIGHT: f64 = 112.0;
/// Écart au coin bas droit de l'écran à la première ouverture (au-dessus de la barre des
/// tâches).
const MARGIN_X: f64 = 24.0;
const MARGIN_Y: f64 = 72.0;
/// Bande du haut de la fenêtre qui doit rester sur un écran pour qu'on puisse l'attraper.
const GRIP: i32 = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct MiniPrefs {
    on_top: bool,
    /// Coin haut gauche en pixels physiques, `None` avant la première fermeture.
    position: Option<(i32, i32)>,
}

impl Default for MiniPrefs {
    fn default() -> Self {
        // Au premier plan par défaut : c'est l'usage attendu d'un mini-lecteur.
        Self {
            on_top: true,
            position: None,
        }
    }
}

/// Rectangle d'un écran en pixels physiques.
#[derive(Debug, Clone, Copy)]
struct Area {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl Area {
    fn of(monitor: &Monitor) -> Self {
        let (p, s) = (monitor.position(), monitor.size());
        Self {
            x: p.x,
            y: p.y,
            w: i32::try_from(s.width).unwrap_or(i32::MAX),
            h: i32::try_from(s.height).unwrap_or(i32::MAX),
        }
    }

    fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// Position gardée seulement si le haut de la fenêtre tombe sur un écran : un écran
/// débranché, ou une fenêtre réduite (-32000 sous Windows), la laisserait hors de vue.
/// Logique pure, testée.
fn reachable(pos: (i32, i32), screens: &[Area]) -> bool {
    let (x, y) = (pos.0.saturating_add(GRIP), pos.1.saturating_add(GRIP / 2));
    screens.iter().any(|s| s.contains(x, y))
}

/// Coin bas droit de l'écran, en pixels physiques. Logique pure, testée.
fn corner(screen: Area, scale: f64) -> (i32, i32) {
    let w = ((WIDTH + MARGIN_X) * scale).round() as i32;
    let h = ((HEIGHT + MARGIN_Y) * scale).round() as i32;
    (screen.x + screen.w - w, screen.y + screen.h - h)
}

fn load(app: &AppHandle) -> MiniPrefs {
    app.try_state::<AppState>()
        .and_then(|state| state.store.clone())
        .and_then(|store| store.get_setting(KEY).ok().flatten())
        .unwrap_or_default()
}

fn save(app: &AppHandle, prefs: &MiniPrefs) {
    let Some(store) = app
        .try_state::<AppState>()
        .and_then(|state| state.store.clone())
    else {
        return;
    };
    if let Err(e) = store.set_setting(KEY, prefs) {
        tracing::warn!("réglages du mini-lecteur non enregistrés : {e}");
    }
}

/// Position de la fenêtre à l'ouverture : celle gardée si elle est encore sur un écran,
/// sinon le coin bas droit de l'écran de la fenêtre principale.
fn place(app: &AppHandle, saved: Option<(i32, i32)>) -> Option<(i32, i32)> {
    let monitors = app.available_monitors().ok()?;
    let screens: Vec<Area> = monitors.iter().map(Area::of).collect();
    if let Some(pos) = saved.filter(|p| reachable(*p, &screens)) {
        return Some(pos);
    }
    let monitor = app
        .get_webview_window("main")
        .and_then(|main| main.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten())?;
    Some(corner(Area::of(&monitor), monitor.scale_factor()))
}

/// Ouvre le mini-lecteur (ou le ramène devant) et cache la fenêtre principale. À appeler
/// hors du thread principal (commande `async`, tâche) : créer une WebView depuis un
/// gestionnaire d'événement bloque sous Windows.
fn open(app: &AppHandle) -> tauri::Result<()> {
    if let Some(mini) = app.get_webview_window(LABEL) {
        mini.unminimize()?;
        mini.show()?;
        mini.set_focus()?;
    } else {
        let prefs = load(app);
        let mini = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::default())
            .title("Seaglass")
            .inner_size(WIDTH, HEIGHT)
            .resizable(false)
            .maximizable(false)
            .decorations(false)
            .always_on_top(prefs.on_top)
            .visible(false)
            .build()?;
        if let Some((x, y)) = place(app, prefs.position) {
            mini.set_position(PhysicalPosition::new(x, y))?;
        }
        mini.show()?;
        mini.set_focus()?;
    }
    if let Some(main) = app.get_webview_window("main") {
        main.hide()?;
    }
    Ok(())
}

/// Ouverture depuis le menu de la zone de notification, qui tourne sur le thread principal.
pub fn open_in_background(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = open(&app) {
            tracing::warn!("mini-lecteur non ouvert : {e}");
        }
    });
}

/// Le mini-lecteur se ferme (son bouton, Alt+F4, fenêtre principale réaffichée) : sa
/// position est gardée et la fenêtre principale revient.
pub(crate) fn on_close_requested(window: &tauri::Window) {
    let app = window.app_handle();
    remember_position(app, window.outer_position());
    tray::reveal_main(app);
}

/// L'app quitte (menu de la zone de notification) avec le mini-lecteur ouvert : sa
/// position est gardée, sa fermeture ne passant pas par `CloseRequested`.
pub(crate) fn on_exit(app: &AppHandle) {
    if let Some(mini) = app.get_webview_window(LABEL) {
        remember_position(app, mini.outer_position());
    }
}

fn remember_position(app: &AppHandle, position: tauri::Result<PhysicalPosition<i32>>) {
    let Ok(p) = position else {
        return;
    };
    let mut prefs = load(app);
    if prefs.position != Some((p.x, p.y)) {
        prefs.position = Some((p.x, p.y));
        save(app, &prefs);
    }
}

#[tauri::command]
pub async fn open_mini_player(app: AppHandle) -> CmdResult<()> {
    open(&app).map_err(err)
}

/// Retour à la fenêtre principale ; la fermeture du mini-lecteur la réaffiche.
#[tauri::command]
pub async fn close_mini_player(app: AppHandle) -> CmdResult<()> {
    tray::show_main(&app);
    Ok(())
}

#[tauri::command]
pub async fn mini_player_on_top(app: AppHandle) -> CmdResult<bool> {
    Ok(load(&app).on_top)
}

/// Règle « Toujours au premier plan » et renvoie l'état retenu.
#[tauri::command]
pub async fn set_mini_player_on_top(app: AppHandle, on_top: bool) -> CmdResult<bool> {
    if let Some(mini) = app.get_webview_window(LABEL) {
        mini.set_always_on_top(on_top).map_err(err)?;
    }
    let mut prefs = load(&app);
    prefs.on_top = on_top;
    save(&app, &prefs);
    Ok(on_top)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEFT: Area = Area {
        x: -1920,
        y: 0,
        w: 1920,
        h: 1080,
    };
    const MAIN: Area = Area {
        x: 0,
        y: 0,
        w: 2560,
        h: 1440,
    };

    #[test]
    fn saved_position_must_stay_on_a_screen() {
        assert!(reachable((100, 100), &[MAIN]));
        assert!(reachable((-1000, 500), &[LEFT, MAIN]));
        // Écran de gauche débranché : la fenêtre serait hors de vue.
        assert!(!reachable((-1000, 500), &[MAIN]));
        // Fenêtre réduite sous Windows.
        assert!(!reachable((-32000, -32000), &[LEFT, MAIN]));
        // Seul un bout du bord droit dépasse : trop peu pour l'attraper.
        assert!(!reachable((2550, 100), &[MAIN]));
        assert!(!reachable((i32::MAX, i32::MAX), &[MAIN]));
    }

    #[test]
    fn first_opening_goes_to_the_bottom_right_corner() {
        assert_eq!(corner(MAIN, 1.0), (2560 - 384, 1440 - 184));
        assert_eq!(corner(MAIN, 1.5), (2560 - 576, 1440 - 276));
        assert_eq!(corner(LEFT, 1.0), (-1920 + 1920 - 384, 1080 - 184));
    }

    #[test]
    fn prefs_default_to_on_top_and_survive_missing_fields() {
        let prefs: MiniPrefs = serde_json::from_str("{}").unwrap();
        assert_eq!(prefs, MiniPrefs::default());
        assert!(prefs.on_top);
        let prefs: MiniPrefs =
            serde_json::from_str(r#"{"onTop":false,"position":[10,20]}"#).unwrap();
        assert_eq!(
            prefs,
            MiniPrefs {
                on_top: false,
                position: Some((10, 20))
            }
        );
    }
}
