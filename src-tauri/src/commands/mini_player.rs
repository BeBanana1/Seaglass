//! Desktop mini player (#384): a small borderless window, optionally always on top,
//! that replaces the main window while it is open. It loads the same UI, which recognizes
//! itself by its window label and only displays `player-state` and sends the player
//! commands. The position and "always on top" are kept in the SQLite
//! settings.
//!
//! Closing the mini player (its button, Alt+F4) shows the main window again; showing the
//! main window again (notification area, relaunching the app) closes the mini player
//! (`tray::show_main`). The two are never visible together.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Monitor, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

use super::{AppState, CmdResult, err};
use crate::integrations::tray;

/// Window label, used by `capabilities/mini.json` and the UI.
pub const LABEL: &str = "mini";
const KEY: &str = "mini_player";
/// Logical window size: the mini player's UI is drawn for it.
const WIDTH: f64 = 360.0;
const HEIGHT: f64 = 112.0;
/// Gap from the screen's bottom right corner on first open (above the
/// taskbar).
const MARGIN_X: f64 = 24.0;
const MARGIN_Y: f64 = 72.0;
/// Top strip of the window that must stay on a screen so that it can be grabbed.
const GRIP: i32 = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct MiniPrefs {
    on_top: bool,
    /// Top left corner in physical pixels, `None` before the first close.
    position: Option<(i32, i32)>,
}

impl Default for MiniPrefs {
    fn default() -> Self {
        // Always on top by default: that is the expected use of a mini player.
        Self {
            on_top: true,
            position: None,
        }
    }
}

/// Rectangle of a screen in physical pixels.
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

/// Position kept only if the top of the window lands on a screen: an unplugged screen,
/// or a minimized window (-32000 on Windows), would leave it out of view.
/// Pure logic, tested.
fn reachable(pos: (i32, i32), screens: &[Area]) -> bool {
    let (x, y) = (pos.0.saturating_add(GRIP), pos.1.saturating_add(GRIP / 2));
    screens.iter().any(|s| s.contains(x, y))
}

/// Bottom right corner of the screen, in physical pixels. Pure logic, tested.
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
        tracing::warn!("mini player settings not saved: {e}");
    }
}

/// Window position on open: the kept one if it is still on a screen,
/// otherwise the bottom right corner of the main window's screen.
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

/// Opens the mini player (or brings it to the front) and hides the main window. Call it
/// off the main thread (`async` command, task): creating a WebView from an event
/// handler blocks on Windows.
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

/// Opening from the notification area menu, which runs on the main thread.
pub fn open_in_background(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = open(&app) {
            tracing::warn!("mini player not opened: {e}");
        }
    });
}

/// The mini player closes (its button, Alt+F4, main window shown again): its
/// position is kept and the main window comes back.
pub(crate) fn on_close_requested(window: &tauri::Window) {
    let app = window.app_handle();
    remember_position(app, window.outer_position());
    tray::reveal_main(app);
}

/// The app quits (notification area menu) with the mini player open: its
/// position is kept, since its closing does not go through `CloseRequested`.
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

/// Back to the main window; closing the mini player shows it again.
#[tauri::command]
pub async fn close_mini_player(app: AppHandle) -> CmdResult<()> {
    tray::show_main(&app);
    Ok(())
}

#[tauri::command]
pub async fn mini_player_on_top(app: AppHandle) -> CmdResult<bool> {
    Ok(load(&app).on_top)
}

/// Sets "Always on top" and returns the state kept.
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
        // Left screen unplugged: the window would be out of view.
        assert!(!reachable((-1000, 500), &[MAIN]));
        // Window minimized on Windows.
        assert!(!reachable((-32000, -32000), &[LEFT, MAIN]));
        // Only a bit of the right edge shows: too little to grab.
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
