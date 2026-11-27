//! Pins as shortcuts on the Android app icon (#335, long press on the icon).
//!
//! Rust sends the list of pins to the Kotlin plugin (`PinShortcuts`) at startup and at
//! each change; the first ones become dynamic shortcuts. A tap (or the
//! `seaglass://pin/<key>` link) comes back here through the session (`request`): the pin is
//! looked up in the settings, kept, then announced to the UI, which reads it like a tile of
//! driving mode (`src/lib/pins.ts`, remembered sort included).

use std::sync::Mutex;

use serde::Serialize;

use super::CmdResult;
use super::settings::pins::Pin;
use crate::i18n::tr;
use crate::store::SmartList;

/// Dynamic shortcuts: launchers show no more than four or five, "Driving mode" shortcut
/// included.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
const MAX_SHORTCUTS: usize = 4;

/// Pin requested by a shortcut, kept until the UI takes it.
static REQUEST: Mutex<Option<Pin>> = Mutex::new(None);

#[derive(Debug, PartialEq, Eq, Serialize)]
struct Shortcut {
    key: String,
    label: String,
}

/// All the pins, in order: Kotlin keeps the first ones as dynamic shortcuts and
/// uses the others so as not to disable a shortcut placed on the home screen.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn shortcuts(pins: &[Pin]) -> Vec<Shortcut> {
    pins.iter()
        .map(|pin| Shortcut {
            key: pin.key().to_owned(),
            label: label(pin),
        })
        .collect()
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn label(pin: &Pin) -> String {
    match pin {
        Pin::Playlist { title, .. } if !title.is_empty() => title.clone(),
        Pin::Playlist { .. } => "Playlist".into(),
        Pin::Mix { title, .. } if !title.is_empty() => title.clone(),
        Pin::Mix { .. } => "Mix".into(),
        Pin::LikedTracks => tr("Titres likés", "Liked tracks").into(),
        Pin::Later => tr("Plus tard", "Later").into(),
        Pin::Smart { list } => match list {
            SmartList::TopMonth => tr("Mes 50 du mois", "My top 50 this month"),
            SmartList::OnRepeat => tr("En boucle en ce moment", "On repeat"),
            SmartList::Forgotten => tr("Oubliés", "Forgotten favorites"),
            SmartList::NeverPlayed => tr("Likés jamais écoutés", "Liked, never played"),
        }
        .into(),
    }
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn find<'a>(pins: &'a [Pin], key: &str) -> Option<&'a Pin> {
    pins.iter().find(|pin| pin.key() == key)
}

/// Sends the pins to the system (Android; nothing on desktop). Without waiting: a failure
/// does not prevent saving the pins, the list is sent again at the next change.
pub fn sync(pins: &[Pin]) {
    #[cfg(target_os = "android")]
    {
        let args = serde_json::json!({
            "pins": shortcuts(pins),
            "max": MAX_SHORTCUTS,
            "removed": tr("Épingle retirée", "Pin removed"),
        });
        tauri::async_runtime::spawn(async move {
            if let Err(e) = tauri_plugin_seaglass_mobile::direct::call::<serde::de::IgnoredAny>(
                "pinShortcuts",
                args,
            )
            .await
            {
                tracing::warn!("pin shortcuts not updated: {e}");
            }
        });
    }
    #[cfg(not(target_os = "android"))]
    let _ = pins;
}

/// Shortcut tapped or `seaglass://pin/<key>` link: the key comes from outside, only an
/// existing pin is accepted.
#[cfg(target_os = "android")]
pub fn request(app: &tauri::AppHandle, key: String) {
    use tauri::{Emitter, Manager};

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let pins = super::settings::load(&app.state::<crate::AppState>()).pins;
        let Some(pin) = find(&pins, &key) else {
            tracing::warn!("shortcut of a missing pin");
            return;
        };
        if let Ok(mut request) = REQUEST.lock() {
            *request = Some(pin.clone());
        }
        if let Err(e) = app.emit("open-pin", ()) {
            tracing::warn!("pin not requested from the UI: {e}");
        }
    });
}

/// Pin requested by a shortcut since the last call; always `None` on desktop.
/// Read at UI startup and on the `open-pin` event.
#[tauri::command]
pub async fn take_pin_request() -> CmdResult<Option<Pin>> {
    Ok(REQUEST.lock().ok().and_then(|mut request| request.take()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playlist(uuid: &str, title: &str) -> Pin {
        Pin::Playlist {
            uuid: uuid.into(),
            title: title.into(),
        }
    }

    #[test]
    fn shortcuts_follow_the_pins() {
        let pins = [
            playlist("36ea71a8-445e-41a4-82ab-6628c581535d", "Route"),
            Pin::LikedTracks,
            Pin::Mix {
                id: "01636e04b6e9ec28485f7d0372b7f3".into(),
                title: String::new(),
            },
            Pin::Later,
        ];
        let list = shortcuts(&pins);
        let keys: Vec<&str> = list.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(
            keys,
            [
                "36ea71a8-445e-41a4-82ab-6628c581535d",
                "liked-tracks",
                "01636e04b6e9ec28485f7d0372b7f3",
                "later"
            ]
        );
        assert_eq!(list[0].label, "Route");
        assert_eq!(list[2].label, "Mix", "empty title: generic label");
        assert!(list.iter().all(|s| !s.label.is_empty()));
    }

    #[test]
    fn only_an_existing_pin_is_found() {
        let pins = [playlist("36ea71a8-445e-41a4-82ab-6628c581535d", "Route")];
        assert_eq!(
            find(&pins, "36ea71a8-445e-41a4-82ab-6628c581535d"),
            Some(&pins[0])
        );
        assert_eq!(find(&pins, "later"), None);
        assert_eq!(find(&pins, "../drive"), None);
    }
}
