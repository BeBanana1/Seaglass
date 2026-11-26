//! Notification area icon (#83): left click = show the window, or hide it if it is already in the
//! foreground (#226); a menu with the current track, Play/Pause, Next, Previous, Show, Mini player
//! (#384), Quit. The commands go through `Cmd`, like SMTC; the menu is kept up to date as an
//! observer of the actor.
//!
//! "Close the window" setting: with `tray`, closing the main window hides it instead of quitting,
//! only if the icon exists (otherwise the app would become unreachable).

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};
use tokio::sync::mpsc;

use super::{NowPlaying, PlaybackStatus};
use crate::commands::mini_player;
use crate::i18n::{self, tr};
use crate::player::{Cmd, PlayerState};

/// Argument added to the `Run` key of the launch at startup: the app starts hidden.
pub const MINIMIZED_ARG: &str = "--minimized";

static CLOSE_TO_TRAY: AtomicBool = AtomicBool::new(false);
static TRAY_READY: AtomicBool = AtomicBool::new(false);
/// Last time the main window lost focus (#226).
static FOCUS_LOST: Mutex<Option<Instant>> = Mutex::new(None);
/// Clicking the icon takes the focus from the window just before the click: a window that lost it
/// less than this ago was still in the foreground.
const FOCUS_GRACE: Duration = Duration::from_millis(500);

/// Focus of the main window, tracked for the click on the icon (#226).
pub fn note_focus(focused: bool) {
    if let Ok(mut lost) = FOCUS_LOST.lock() {
        *lost = (!focused).then(Instant::now);
    }
}

/// Click on the icon: hide a visible window in the foreground, otherwise show it (hidden, minimized
/// or behind other windows). Pure logic, tested.
fn click_hides(visible: bool, minimized: bool, focused: bool, lost_ago: Option<Duration>) -> bool {
    visible && !minimized && (focused || lost_ago.is_some_and(|d| d <= FOCUS_GRACE))
}

/// Left click on the icon: show or hide the main window (#226).
fn toggle_main(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let lost_ago = FOCUS_LOST
        .lock()
        .ok()
        .and_then(|lost| lost.map(|at| at.elapsed()));
    let hides = click_hides(
        window.is_visible().unwrap_or(false),
        window.is_minimized().unwrap_or(false),
        window.is_focused().unwrap_or(false),
        lost_ago,
    );
    if hides {
        let _ = window.hide();
    } else {
        show_main(app);
    }
}

/// "Close the window" setting: true = minimize to the notification area.
pub fn set_close_to_tray(enabled: bool) {
    CLOSE_TO_TRAY.store(enabled, Ordering::Relaxed);
}

/// Should closing the main window hide it rather than quit?
pub fn hide_on_close() -> bool {
    CLOSE_TO_TRAY.load(Ordering::Relaxed) && TRAY_READY.load(Ordering::Relaxed)
}

/// Started by the `Run` key with `--minimized`, and the icon exists: the window stays hidden.
pub fn start_hidden(args: impl IntoIterator<Item = String>) -> bool {
    TRAY_READY.load(Ordering::Relaxed) && args.into_iter().any(|a| a == MINIMIZED_ARG)
}

/// Seaglass started again while already running: the existing window is shown again, except for a
/// launch at session start (`--minimized`), which must show nothing.
pub fn show_on_second_launch(args: impl IntoIterator<Item = String>) -> bool {
    !args.into_iter().any(|a| a == MINIMIZED_ARG)
}

/// Shows the main window. The mini player (#384) replaces it: it closes, and closing it shows the
/// main window again too.
pub fn show_main(app: &AppHandle) {
    if let Some(mini) = app.get_webview_window(mini_player::LABEL) {
        let _ = mini.close();
    }
    reveal_main(app);
}

/// Shows, restores and brings the main window to the foreground, without touching the mini player.
pub fn reveal_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Menu texts and states for a player state (pure logic, tested).
#[derive(Debug, Clone, PartialEq)]
struct TrayView {
    now: String,
    tooltip: String,
    play_pause: &'static str,
    has_track: bool,
    has_next: bool,
    has_previous: bool,
    show: &'static str,
    mini: &'static str,
    quit: &'static str,
    next: &'static str,
    previous: &'static str,
}

impl TrayView {
    fn from_player(state: &PlayerState) -> Self {
        let np = NowPlaying::from_player(state);
        let (now, tooltip) = match &np.media {
            Some(m) if m.artist.is_empty() => (m.title.clone(), format!("Seaglass\n{}", m.title)),
            Some(m) => (
                format!("{} · {}", m.title, m.artist),
                format!("Seaglass\n{}\n{}", m.title, m.artist),
            ),
            None => (
                tr("Rien en lecture", "Nothing playing").to_owned(),
                "Seaglass".to_owned(),
            ),
        };
        let playing = matches!(
            np.status,
            PlaybackStatus::Playing | PlaybackStatus::Changing
        );
        Self {
            now: ellipsize(&now, 60),
            // Windows truncates an icon's tooltip to 127 characters.
            tooltip: ellipsize(&tooltip, 120),
            play_pause: if playing {
                tr("Pause", "Pause")
            } else {
                tr("Lecture", "Play")
            },
            has_track: np.media.is_some(),
            has_next: np.has_next,
            has_previous: np.has_previous,
            show: tr("Afficher Seaglass", "Show Seaglass"),
            mini: tr("Mini-lecteur", "Mini player"),
            quit: tr("Quitter", "Quit"),
            next: tr("Suivant", "Next"),
            previous: tr("Précédent", "Previous"),
        }
    }
}

/// Cuts to `max` characters (not bytes) with an ellipsis.
fn ellipsize(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(max.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

/// Icon created: notified by `Observers` on every state the actor emits.
pub struct TrayHandle {
    tray: TrayIcon<Wry>,
    now: MenuItem<Wry>,
    play_pause: MenuItem<Wry>,
    next: MenuItem<Wry>,
    previous: MenuItem<Wry>,
    show: MenuItem<Wry>,
    mini: MenuItem<Wry>,
    quit: MenuItem<Wry>,
    /// Last view applied: the menu is only touched when it changes (the state arrives at ~4 Hz
    /// during playback).
    last: Mutex<Option<(TrayView, i18n::Lang)>>,
}

impl TrayHandle {
    pub fn update(&self, state: &PlayerState) {
        let view = TrayView::from_player(state);
        let key = (view.clone(), i18n::current());
        let Ok(mut last) = self.last.lock() else {
            return;
        };
        if last.as_ref() == Some(&key) {
            return;
        }
        if let Err(e) = self.apply(&view) {
            tracing::warn!("notification area: {e}");
        }
        *last = Some(key);
    }

    fn apply(&self, v: &TrayView) -> tauri::Result<()> {
        self.now.set_text(&v.now)?;
        self.play_pause.set_text(v.play_pause)?;
        self.play_pause.set_enabled(v.has_track)?;
        self.next.set_text(v.next)?;
        self.next.set_enabled(v.has_next)?;
        self.previous.set_text(v.previous)?;
        self.previous.set_enabled(v.has_previous)?;
        self.show.set_text(v.show)?;
        self.mini.set_text(v.mini)?;
        self.quit.set_text(v.quit)?;
        self.tray.set_tooltip(Some(&v.tooltip))
    }
}

/// Creates the icon and its menu. On failure, closing the window quits the app as before.
pub fn create(app: &AppHandle, player: mpsc::Sender<Cmd>) -> tauri::Result<TrayHandle> {
    let v = TrayView::from_player(&PlayerState::default());
    let now = MenuItem::with_id(app, "now", &v.now, false, None::<&str>)?;
    let play_pause = MenuItem::with_id(app, "play-pause", v.play_pause, false, None::<&str>)?;
    let next = MenuItem::with_id(app, "next", v.next, false, None::<&str>)?;
    let previous = MenuItem::with_id(app, "previous", v.previous, false, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", v.show, true, None::<&str>)?;
    let mini = MenuItem::with_id(app, "mini", v.mini, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", v.quit, true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &now,
            &PredefinedMenuItem::separator(app)?,
            &play_pause,
            &next,
            &previous,
            &PredefinedMenuItem::separator(app)?,
            &show,
            &mini,
            &quit,
        ],
    )?;
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip(&v.tooltip)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| {
            let cmd = match event.id().as_ref() {
                "play-pause" => Cmd::TogglePause,
                "next" => Cmd::Next,
                "previous" => Cmd::Previous,
                "show" => return show_main(app),
                "mini" => return mini_player::open_in_background(app),
                "quit" => return app.exit(0),
                _ => return,
            };
            if player.try_send(cmd).is_err() {
                tracing::warn!("notification area: player unavailable");
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let tray = builder.build(app)?;
    TRAY_READY.store(true, Ordering::Relaxed);
    Ok(TrayHandle {
        tray,
        now,
        play_pause,
        next,
        previous,
        show,
        mini,
        quit,
        last: Mutex::new(None),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::{AlbumRef, ArtistRef, Track, TrackId};

    fn playing() -> PlayerState {
        PlayerState {
            track: Some(Track {
                id: TrackId(1),
                title: "Time".into(),
                version: None,
                duration: 276,
                artists: vec![ArtistRef {
                    id: 1,
                    name: "Hans Zimmer".into(),
                }],
                album: AlbumRef {
                    id: 2,
                    title: "Inception".into(),
                    cover_url: None,
                    cover_small_url: None,
                    cover_large_url: None,
                    color: None,
                },
                explicit: false,
                ai: false,
                hires: false,
                isrc: None,
            }),
            has_next: true,
            ..PlayerState::default()
        }
    }

    #[test]
    fn menu_follows_the_player() {
        let v = TrayView::from_player(&PlayerState::default());
        assert_eq!(v.now, "Rien en lecture");
        assert_eq!(v.play_pause, "Lecture");
        assert!(!v.has_track && !v.has_next);

        let mut s = playing();
        let v = TrayView::from_player(&s);
        assert_eq!(v.now, "Time · Hans Zimmer");
        assert_eq!(v.tooltip, "Seaglass\nTime\nHans Zimmer");
        assert_eq!(v.play_pause, "Pause");
        assert!(v.has_track && v.has_next);
        // A loaded track always allows "Previous" (back to the start).
        assert!(v.has_previous);

        s.paused = true;
        assert_eq!(TrayView::from_player(&s).play_pause, "Lecture");
    }

    #[test]
    fn long_titles_are_cut_on_characters() {
        let long = "é".repeat(80);
        let cut = ellipsize(&long, 60);
        assert_eq!(cut.chars().count(), 60);
        assert!(cut.ends_with('…'));
        assert_eq!(ellipsize("court", 60), "court");
    }

    #[test]
    fn click_hides_only_a_window_in_front() {
        let just = Some(Duration::from_millis(80));
        // In the foreground, or it just lost the focus because of the click: hidden.
        assert!(click_hides(true, false, true, None));
        assert!(click_hides(true, false, false, just));
        // Behind other windows for a long time, minimized or hidden: shown.
        assert!(!click_hides(
            true,
            false,
            false,
            Some(Duration::from_secs(5))
        ));
        assert!(!click_hides(true, false, false, None));
        assert!(!click_hides(true, true, false, just));
        assert!(!click_hides(false, false, false, just));
    }

    #[test]
    fn second_launch_shows_the_window_unless_minimized() {
        assert!(show_on_second_launch(["seaglass.exe".into()]));
        assert!(!show_on_second_launch([
            "seaglass.exe".into(),
            MINIMIZED_ARG.into()
        ]));
    }

    #[test]
    fn minimized_start_needs_the_argument_and_the_icon() {
        // The icon does not exist in the tests: never hidden.
        assert!(!start_hidden(["seaglass.exe".into(), MINIMIZED_ARG.into()]));
        assert!(!hide_on_close());
    }
}
