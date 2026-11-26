//! Boutons Précédent / Lecture-Pause / Suivant sous la miniature de la fenêtre dans la
//! barre des tâches de Windows (#333), par `ITaskbarList3::ThumbBarAddButtons`.
//!
//! Les clics arrivent en `WM_COMMAND` sur la fenêtre principale : elle est sous-classée
//! (`SetWindowSubclass`, qui cohabite avec le sous-classement de tao) depuis le thread
//! principal, qui la possède. Tout l'état Win32 vit dans un `thread_local` de ce thread ;
//! l'observateur de l'acteur ne lui passe une vue (`run_on_main_thread`) que si elle change.

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

/// Identifiant du sous-classement, propre à ce module.
const SUBCLASS_ID: usize = 0x5EA_333;

/// Boutons, dans l'ordre d'affichage ; leur identifiant est leur rang.
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

/// Ce que montrent les boutons (logique pure, testée).
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

    /// Info-bulle, dans la langue de la vue (`tr` lit la langue courante, la même).
    fn tooltip(&self, button: Button) -> &'static str {
        match button {
            Button::Previous => tr("Précédent", "Previous"),
            Button::PlayPause if self.playing => tr("Pause", "Pause"),
            Button::PlayPause => tr("Lecture", "Play"),
            Button::Next => tr("Suivant", "Next"),
        }
    }
}

/// Observateur de l'acteur : ne réveille le thread principal que si la vue change (l'état
/// arrive à ~4 Hz pendant la lecture).
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
/// Message `TaskbarButtonCreated` : bouton de la fenêtre créé (affichage, redémarrage de
/// l'explorateur). Les boutons de la miniature doivent alors être posés à nouveau.
static BUTTON_CREATED: OnceLock<u32> = OnceLock::new();

thread_local! {
    static BAR: RefCell<Option<Bar>> = const { RefCell::new(None) };
}

/// Accès à l'état du thread principal ; ignoré s'il est déjà emprunté (message reçu
/// pendant un appel à la barre des tâches) ou absent (fenêtre détruite).
fn with_bar(f: impl FnOnce(&mut Bar)) {
    BAR.with(|bar| {
        if let Ok(mut bar) = bar.try_borrow_mut()
            && let Some(bar) = bar.as_mut()
        {
            f(bar);
        }
    });
}

/// Branche les boutons sur la fenêtre `hwnd` (valeur brute du handle). À appeler sur le
/// thread principal, avant le premier affichage de la fenêtre.
pub fn start(
    app: &AppHandle<Wry>,
    hwnd: isize,
    player: mpsc::Sender<Cmd>,
) -> Result<ThumbBarHandle, String> {
    let hwnd = HWND(hwnd as *mut _);
    let _ = PLAYER.set(player);
    let created = *BUTTON_CREATED
        .get_or_init(|| unsafe { RegisterWindowMessageW(w!("TaskbarButtonCreated")) });
    // Déjà initialisé par tao (STA) : sans conséquence.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let list: ITaskbarList3 = unsafe { CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER) }
        .map_err(|e| e.to_string())?;
    unsafe { list.HrInit() }.map_err(|e| e.to_string())?;
    // Lancée en administrateur, la fenêtre filtrerait les messages de l'explorateur.
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
        return Err("sous-classement de la fenêtre impossible".to_owned());
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
        // Thème clair ou sombre changé (`ImmersiveColorSet`) : `refresh` ne redessine les
        // icônes que si le thème de la barre des tâches a vraiment changé.
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
    // Thread de l'interface : pas d'attente possible, la file de l'acteur suffit.
    if PLAYER.get().is_none_or(|p| p.try_send(cmd).is_err()) {
        tracing::warn!("miniature : lecteur indisponible");
    }
}

/// État Win32, sur le thread principal seulement.
struct Bar {
    hwnd: HWND,
    list: ITaskbarList3,
    /// Boutons posés sur le bouton actuel de la barre des tâches.
    added: bool,
    view: ThumbView,
    icons: Option<Icons>,
    /// Jeu précédent, gardé un tour : la barre des tâches peut encore l'afficher.
    retired: Option<Icons>,
}

impl Bar {
    fn refresh(&mut self) {
        let light = taskbar_is_light();
        if self.icons.as_ref().is_none_or(|i| i.light != light) {
            match Icons::new(light) {
                Ok(icons) => self.retired = self.icons.replace(icons),
                Err(e) => {
                    tracing::warn!("miniature : icônes : {e}");
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
            // Avant le premier affichage, le bouton de la barre des tâches n'existe pas :
            // `TaskbarButtonCreated` reposera les boutons.
            Err(e) => tracing::debug!("miniature : {e}"),
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
        // Un titre chargé permet toujours « Précédent » (retour au début).
        assert!(v.enabled(Button::Previous));
        assert!(!v.enabled(Button::Next));

        s.has_next = true;
        s.paused = true;
        let paused = ThumbView::from_player(&s, Lang::Fr);
        assert_eq!(paused.glyph(Button::PlayPause), Glyph::Play);
        assert!(paused.enabled(Button::Next));
        assert_ne!(paused, v);
        // Seule la position change : même vue, rien n'est envoyé au thread principal.
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
