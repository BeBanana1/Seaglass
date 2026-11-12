//! Plein écran de la fenêtre principale (#131), bureau seulement : Android est déjà en
//! plein écran. L'UI ne fait que demander et afficher l'état renvoyé ici.
//!
//! Sous Windows, une fenêtre sans bordure qui couvre exactement l'écran est basculée par DWM
//! en « independent flip » après ~2 s quand elle a le focus, et WebView2 y prend une image de
//! retard (#191). On déborde donc d'un pixel hors de l'écran : la fenêtre le couvre toujours
//! (barre des tâches masquée) sans lui correspondre exactement.

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
    #[cfg(windows)]
    if state && let Err(e) = overflow(&window) {
        tracing::warn!("plein écran : débordement d'un pixel impossible : {e}");
    }
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

/// Agrandit la fenêtre (déjà en plein écran natif) d'un pixel vers un bord de son écran.
/// tao ne la recale sur l'écran que si elle change d'écran, ce qui n'est pas le cas ici.
#[cfg(windows)]
fn overflow(window: &WebviewWindow) -> tauri::Result<()> {
    use tauri::{PhysicalPosition, PhysicalSize};

    let Some(monitor) = window.current_monitor()? else {
        return Ok(());
    };
    let screen = Rect::of(monitor.position(), monitor.size());
    let others: Vec<Rect> = window
        .available_monitors()?
        .iter()
        .map(|m| Rect::of(m.position(), m.size()))
        .filter(|r| *r != screen)
        .collect();
    let r = overflowed(screen, &others);
    window.set_position(PhysicalPosition::new(r.x, r.y))?;
    window.set_size(PhysicalSize::new(r.w, r.h))
}

/// Rectangle d'écran en pixels physiques.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rect {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
}

impl Rect {
    #[cfg(windows)]
    fn of(p: &tauri::PhysicalPosition<i32>, s: &tauri::PhysicalSize<u32>) -> Self {
        Self {
            x: p.x,
            y: p.y,
            w: s.width,
            h: s.height,
        }
    }

    fn right(&self) -> i64 {
        i64::from(self.x) + i64::from(self.w)
    }

    fn bottom(&self) -> i64 {
        i64::from(self.y) + i64::from(self.h)
    }

    fn intersects(&self, o: &Rect) -> bool {
        i64::from(self.x) < o.right()
            && i64::from(o.x) < self.right()
            && i64::from(self.y) < o.bottom()
            && i64::from(o.y) < self.bottom()
    }
}

/// Écran agrandi d'un pixel vers le premier bord (bas, haut, droite, gauche) dont la bande
/// d'un pixel ne tombe sur aucun autre écran ; le bas si tous ont un voisin.
#[cfg_attr(not(windows), allow(dead_code))]
fn overflowed(s: Rect, others: &[Rect]) -> Rect {
    let (bottom_y, right_x) = (
        s.y.saturating_add_unsigned(s.h),
        s.x.saturating_add_unsigned(s.w),
    );
    let candidates = [
        (
            Rect {
                y: bottom_y,
                h: 1,
                ..s
            },
            Rect { h: s.h + 1, ..s },
        ),
        (
            Rect {
                y: s.y - 1,
                h: 1,
                ..s
            },
            Rect {
                y: s.y - 1,
                h: s.h + 1,
                ..s
            },
        ),
        (
            Rect {
                x: right_x,
                w: 1,
                ..s
            },
            Rect { w: s.w + 1, ..s },
        ),
        (
            Rect {
                x: s.x - 1,
                w: 1,
                ..s
            },
            Rect {
                x: s.x - 1,
                w: s.w + 1,
                ..s
            },
        ),
    ];
    candidates
        .iter()
        .find(|(strip, _)| !others.iter().any(|o| strip.intersects(o)))
        .map_or(candidates[0].1, |(_, grown)| *grown)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn single_screen_grows_down() {
        assert_eq!(overflowed(r(0, 0, 1920, 1080), &[]), r(0, 0, 1920, 1081));
    }

    #[test]
    fn screen_below_grows_up() {
        let others = [r(0, 1080, 1920, 1080)];
        assert_eq!(
            overflowed(r(0, 0, 1920, 1080), &others),
            r(0, -1, 1920, 1081)
        );
    }

    #[test]
    fn side_by_side_grows_down() {
        let others = [r(1920, 0, 2560, 1440)];
        assert_eq!(
            overflowed(r(0, 0, 1920, 1080), &others),
            r(0, 0, 1920, 1081)
        );
    }

    #[test]
    fn lower_neighbour_on_the_right_leaves_bottom_free() {
        // L'écran de droite descend plus bas, mais il commence à x = 1920 : la bande du bas
        // (x de 0 à 1919) ne le touche pas.
        let others = [r(1920, 200, 1920, 1080)];
        assert_eq!(
            overflowed(r(0, 0, 1920, 1080), &others),
            r(0, 0, 1920, 1081)
        );
    }

    #[test]
    fn surrounded_vertically_grows_right() {
        let others = [r(0, 1080, 1920, 1080), r(0, -1080, 1920, 1080)];
        assert_eq!(
            overflowed(r(0, 0, 1920, 1080), &others),
            r(0, 0, 1921, 1080)
        );
    }

    #[test]
    fn surrounded_everywhere_falls_back_down() {
        let others = [
            r(0, 1080, 1920, 1080),
            r(0, -1080, 1920, 1080),
            r(1920, 0, 1920, 1080),
            r(-1920, 0, 1920, 1080),
        ];
        assert_eq!(
            overflowed(r(0, 0, 1920, 1080), &others),
            r(0, 0, 1920, 1081)
        );
    }
}
