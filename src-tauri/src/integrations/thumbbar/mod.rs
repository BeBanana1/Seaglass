//! Previous / Play-Pause / Next buttons under the window's thumbnail in the Windows
//! taskbar (#333), through `ITaskbarList3::ThumbBarAddButtons`.
//!
//! Clicks arrive as `WM_COMMAND` on the main window: it is subclassed (`SetWindowSubclass`,
//! which coexists with tao's subclassing) from the main thread, which owns it. All the Win32
//! state lives in a `thread_local` of that thread; the actor's observer only hands it a view
//! (`run_on_main_thread`) when the view changes.

mod glyphs;
mod icons;

use std::cell::RefCell;
use std::sync::{Mutex, OnceLock};

use tauri::{AppHandle, Wry};
use tokio::sync::mpsc;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::UI::Shell::{
    DefSubclassProc, ITaskbarList3, RemoveWindowSubclass, SetWindowSubclass, THB_FLAGS, THB_ICON,
    THB_TOOLTIP, THBF_DISABLED, THBF_ENABLED, THBN_CLICKED, THUMBBUTTON, TaskbarList,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ChangeWindowMessageFilterEx, MSGFLT_ALLOW, RegisterWindowMessageW, WM_COMMAND, WM_NCDESTROY,
    WM_SETTINGCHANGE,
};
use windows::core::w;

use self::glyphs::Glyph;
use self::icons::{Icons, taskbar_is_light};
use super::{NowPlaying, PlaybackStatus};
use crate::i18n::{self, Lang, tr};
use crate::player::{Cmd, PlayerState};

/// Subclass identifier, specific to this module.
const SUBCLASS_ID: usize = 0x5EA_333;

/// Buttons, in display order; their identifier is their index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Button {
    Previous,
    PlayPause,
    Next,
}

const BUTTONS: [Button; 3] = [Button::Previous, Button::PlayPause, Button::Next];

impl Button {
    fn id(self) -> u32 {
        self as u32
    }

    fn from_id(id: u32) -> Option<Self> {
        BUTTONS.into_iter().find(|b| b.id() == id)
    }
}

/// What the buttons show (pure logic, tested).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ThumbView {
    playing: bool,
    has_track: bool,
    has_next: bool,
    has_previous: bool,
    lang: Lang,
}

impl ThumbView {
    fn from_player(state: &PlayerState, lang: Lang) -> Self {
        let np = NowPlaying::from_player(state);
        Self {
            playing: matches!(
                np.status,
                PlaybackStatus::Playing | PlaybackStatus::Changing
            ),
            has_track: np.media.is_some(),
            has_next: np.has_next,
            has_previous: np.has_previous,
            lang,
        }
    }

    fn enabled(&self, button: Button) -> bool {
        match button {
            Button::Previous => self.has_previous,
            Button::PlayPause => self.has_track,
            Button::Next => self.has_next,
        }
    }

    fn glyph(&self, button: Button) -> Glyph {
        match button {
            Button::Previous => Glyph::Previous,
            Button::PlayPause if self.playing => Glyph::Pause,
            Button::PlayPause => Glyph::Play,
            Button::Next => Glyph::Next,
        }
    }

    /// Tooltip, in the view's language (`tr` reads the current language, the same one).
    fn tooltip(&self, button: Button) -> &'static str {
        match button {
            Button::Previous => tr("Précédent", "Previous"),
            Button::PlayPause if self.playing => tr("Pause", "Pause"),
            Button::PlayPause => tr("Lecture", "Play"),
            Button::Next => tr("Suivant", "Next"),
        }
    }
}

/// The actor's observer: only wakes the main thread up when the view changes (the state
/// arrives at ~4 Hz during playback).
pub struct ThumbBarHandle {
    app: AppHandle<Wry>,
    last: Mutex<Option<ThumbView>>,
}

impl ThumbBarHandle {
    pub fn update(&self, state: &PlayerState) {
        let view = ThumbView::from_player(state, i18n::current());
        let Ok(mut last) = self.last.lock() else {
            return;
        };
        if *last == Some(view) {
            return;
        }
        *last = Some(view);
        let _ = self.app.run_on_main_thread(move || {
            with_bar(|bar| {
                bar.view = view;
                bar.refresh();
            });
        });
    }
}

static PLAYER: OnceLock<mpsc::Sender<Cmd>> = OnceLock::new();
/// `TaskbarButtonCreated` message: the window's taskbar button was created (first display,
/// Explorer restart). The thumbnail buttons must then be added again.
static BUTTON_CREATED: OnceLock<u32> = OnceLock::new();

thread_local! {
    static BAR: RefCell<Option<Bar>> = const { RefCell::new(None) };
}

/// Access to the main thread's state; skipped if it is already borrowed (message received
/// during a call to the taskbar) or missing (window destroyed).
fn with_bar(f: impl FnOnce(&mut Bar)) {
    BAR.with(|bar| {
        if let Ok(mut bar) = bar.try_borrow_mut()
            && let Some(bar) = bar.as_mut()
        {
            f(bar);
        }
    });
}

/// Attaches the buttons to the window `hwnd` (raw handle value). To call on the main
/// thread, before the window is first shown.
pub fn start(
    app: &AppHandle<Wry>,
    hwnd: isize,
    player: mpsc::Sender<Cmd>,
) -> Result<ThumbBarHandle, String> {
    let hwnd = HWND(hwnd as *mut _);
    let _ = PLAYER.set(player);
    let created = *BUTTON_CREATED
        .get_or_init(|| unsafe { RegisterWindowMessageW(w!("TaskbarButtonCreated")) });
    // Already initialized by tao (STA): harmless.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let list: ITaskbarList3 = unsafe { CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER) }
        .map_err(|e| e.to_string())?;
    unsafe { list.HrInit() }.map_err(|e| e.to_string())?;
    // Run as administrator, the window would filter out Explorer's messages.
    unsafe {
        let _ = ChangeWindowMessageFilterEx(hwnd, created, MSGFLT_ALLOW, None);
        let _ = ChangeWindowMessageFilterEx(hwnd, WM_COMMAND, MSGFLT_ALLOW, None);
    }
    BAR.with(|bar| {
        *bar.borrow_mut() = Some(Bar {
            hwnd,
            list,
            added: false,
            view: ThumbView::from_player(&PlayerState::default(), i18n::current()),
            icons: None,
            retired: None,
        });
    });
    if !unsafe { SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, 0) }.as_bool() {
        BAR.with(|bar| bar.borrow_mut().take());
        return Err("could not subclass the window".to_owned());
    }
    Ok(ThumbBarHandle {
        app: app.clone(),
        last: Mutex::new(None),
    })
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if msg == WM_COMMAND && (wparam.0 >> 16) as u32 & 0xFFFF == THBN_CLICKED {
        if let Some(button) = Button::from_id((wparam.0 & 0xFFFF) as u32) {
            clicked(button);
            return LRESULT(0);
        }
    } else if Some(&msg) == BUTTON_CREATED.get() {
        with_bar(|bar| {
            bar.added = false;
            bar.refresh();
        });
    } else if msg == WM_SETTINGCHANGE {
        // Light or dark theme changed (`ImmersiveColorSet`): `refresh` only redraws the
        // icons if the taskbar theme really changed.
        with_bar(Bar::refresh);
    } else if msg == WM_NCDESTROY {
        BAR.with(|bar| {
            if let Ok(mut bar) = bar.try_borrow_mut() {
                bar.take();
            }
        });
        let _ = unsafe { RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID) };
    }
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

fn clicked(button: Button) {
    let cmd = match button {
        Button::Previous => Cmd::Previous,
        Button::PlayPause => Cmd::TogglePause,
        Button::Next => Cmd::Next,
    };
    // UI thread: no waiting allowed, the actor's queue is enough.
    if PLAYER.get().is_none_or(|p| p.try_send(cmd).is_err()) {
        tracing::warn!("thumbbar: player unavailable");
    }
}

/// Win32 state, on the main thread only.
struct Bar {
    hwnd: HWND,
    list: ITaskbarList3,
    /// Buttons added to the current taskbar button.
    added: bool,
    view: ThumbView,
    icons: Option<Icons>,
    /// Previous set, kept for one round: the taskbar may still show it.
    retired: Option<Icons>,
}

impl Bar {
    fn refresh(&mut self) {
        let light = taskbar_is_light();
        if self.icons.as_ref().is_none_or(|i| i.light != light) {
            match Icons::new(light) {
                Ok(icons) => self.retired = self.icons.replace(icons),
                Err(e) => {
                    tracing::warn!("thumbbar: icons: {e}");
                    return;
                }
            }
        }
        let Some(icons) = &self.icons else {
            return;
        };
        let buttons = BUTTONS.map(|b| thumb_button(b, &self.view, icons));
        let result = if self.added {
            unsafe { self.list.ThumbBarUpdateButtons(self.hwnd, &buttons) }
        } else {
            unsafe { self.list.ThumbBarAddButtons(self.hwnd, &buttons) }
        };
        match result {
            Ok(()) => self.added = true,
            // Before the first display, the taskbar button does not exist:
            // `TaskbarButtonCreated` will add the buttons again.
            Err(e) => tracing::debug!("thumbbar: {e}"),
        }
    }
}

fn thumb_button(button: Button, view: &ThumbView, icons: &Icons) -> THUMBBUTTON {
    let mut tip = [0u16; 260];
    for (slot, unit) in tip
        .iter_mut()
        .take(259)
        .zip(view.tooltip(button).encode_utf16())
    {
        *slot = unit;
    }
    THUMBBUTTON {
        dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS,
        iId: button.id(),
        iBitmap: 0,
        hIcon: icons.get(view.glyph(button)),
        szTip: tip,
        dwFlags: if view.enabled(button) {
            THBF_ENABLED
        } else {
            THBF_DISABLED
        },
    }
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
            ..PlayerState::default()
        }
    }

    #[test]
    fn nothing_loaded_disables_every_button() {
        let v = ThumbView::from_player(&PlayerState::default(), Lang::Fr);
        assert!(BUTTONS.iter().all(|&b| !v.enabled(b)));
        assert_eq!(v.glyph(Button::PlayPause), Glyph::Play);
    }

    #[test]
    fn buttons_follow_the_player() {
        let mut s = playing();
        let v = ThumbView::from_player(&s, Lang::Fr);
        assert_eq!(v.glyph(Button::PlayPause), Glyph::Pause);
        assert!(v.enabled(Button::PlayPause));
        // A loaded track always allows "Previous" (back to the start).
        assert!(v.enabled(Button::Previous));
        assert!(!v.enabled(Button::Next));

        s.has_next = true;
        s.paused = true;
        let paused = ThumbView::from_player(&s, Lang::Fr);
        assert_eq!(paused.glyph(Button::PlayPause), Glyph::Play);
        assert!(paused.enabled(Button::Next));
        assert_ne!(paused, v);
        // Only the position changes: same view, nothing is sent to the main thread.
        s.time_pos = 42.0;
        assert_eq!(ThumbView::from_player(&s, Lang::Fr), paused);
    }

    #[test]
    fn button_ids_round_trip() {
        for b in BUTTONS {
            assert_eq!(Button::from_id(b.id()), Some(b));
        }
        assert_eq!(Button::from_id(7), None);
    }
}
