//! Full screen of the main window (#131), desktop only: Android is already in
//! full screen. The UI only asks and displays the state returned here.
//!
//! On Windows, a borderless window that covers the screen exactly is switched by DWM
//! to "independent flip" after ~2 s when it has focus, and WebView2 lags one frame
//! behind (#191). So it overflows by one pixel off the screen: the window still covers it
//! (taskbar hidden) without matching it exactly.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter, Manager, WebviewWindow, Window};

use super::{CmdResult, err};
use crate::i18n::tr;

const MAIN: &str = "main";

/// Last state sent to the UI, to emit `fullscreen` only when it changes.
static FULLSCREEN: AtomicBool = AtomicBool::new(false);

fn main_window(app: &AppHandle) -> CmdResult<WebviewWindow> {
    app.get_webview_window(MAIN)
        .ok_or_else(|| tr("Fenêtre principale introuvable", "Main window not found").to_string())
}

#[tauri::command]
pub async fn fullscreen(app: AppHandle) -> CmdResult<bool> {
    main_window(&app)?.is_fullscreen().map_err(err)
}

/// Puts the window in full screen or takes it out, then returns the state read back.
#[tauri::command]
pub async fn set_fullscreen(app: AppHandle, fullscreen: bool) -> CmdResult<bool> {
    let window = main_window(&app)?;
    window.set_fullscreen(fullscreen).map_err(err)?;
    let state = window.is_fullscreen().map_err(err)?;
    #[cfg(windows)]
    if state && let Err(e) = overflow(&window) {
        tracing::warn!("full screen: one pixel overflow impossible: {e}");
    }
    notify(&app, state);
    Ok(state)
}

/// Called on every resize: the state can also change outside the app.
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

/// Enlarges the window (already in native full screen) by one pixel towards an edge of its
/// screen. tao only re-fits it to the screen when it changes screen, not the case here.
///
/// Direct `SetWindowPos` rather than `set_position` / `set_size`: tao's clear its
/// "maximized" flag, so leaving full screen went through the restored window before
/// maximizing. Called on the window's thread, after switching to full screen.
#[cfg(windows)]
fn overflow(window: &WebviewWindow) -> tauri::Result<()> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOZORDER, SetWindowPos,
    };

    let hwnd = window.hwnd()?.0 as isize;
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
    let (w, h) = (
        i32::try_from(r.w).unwrap_or(i32::MAX),
        i32::try_from(r.h).unwrap_or(i32::MAX),
    );
    window.run_on_main_thread(move || {
        let flags = SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER;
        // SAFETY: `hwnd` is the main window, alive as long as the app runs.
        if let Err(e) = unsafe { SetWindowPos(HWND(hwnd as *mut _), None, r.x, r.y, w, h, flags) } {
            tracing::warn!("full screen: SetWindowPos failed: {e}");
        }
    })
}

/// Screen rectangle in physical pixels.
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

/// Screen enlarged by one pixel towards the first edge (bottom, top, right, left) whose
/// one pixel strip lands on no other screen; the bottom if all have a neighbor.
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
        // The right screen goes lower, but it starts at x = 1920: the bottom strip
        // (x from 0 to 1919) does not touch it.
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
