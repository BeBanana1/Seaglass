//! Épingles en raccourcis de l'icône de l'app Android (#335, appui long sur l'icône).
//!
//! Rust envoie la liste des épingles au plugin Kotlin (`PinShortcuts`) au démarrage et à
//! chaque changement ; les premières deviennent des raccourcis dynamiques. Un appui (ou le
//! lien `seaglass://pin/<clé>`) revient ici par la session (`request`) : l'épingle est
//! cherchée dans les réglages, gardée, puis annoncée à l'UI, qui la lit comme une tuile du
//! mode conduite (`src/lib/pins.ts`, tri mémorisé compris).

use std::sync::Mutex;

use serde::Serialize;

use super::CmdResult;
use super::settings::pins::Pin;
use crate::i18n::tr;

/// Raccourcis dynamiques : les lanceurs n'en affichent pas plus de quatre ou cinq, raccourci
/// « Mode conduite » compris.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
const MAX_SHORTCUTS: usize = 4;

/// Épingle demandée par un raccourci, gardée jusqu'à ce que l'UI la prenne.
static REQUEST: Mutex<Option<Pin>> = Mutex::new(None);

#[derive(Debug, PartialEq, Eq, Serialize)]
struct Shortcut {
    key: String,
    label: String,
}

/// Toutes les épingles, dans l'ordre : Kotlin garde les premières en raccourcis dynamiques et
/// se sert des autres pour ne pas désactiver un raccourci posé sur l'écran d'accueil.
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
    }
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn find<'a>(pins: &'a [Pin], key: &str) -> Option<&'a Pin> {
    pins.iter().find(|pin| pin.key() == key)
}

/// Envoie les épingles au système (Android ; rien sur le bureau). Sans attendre : un échec
/// n'empêche pas d'enregistrer les épingles, la liste repart au prochain changement.
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
                tracing::warn!("raccourcis des épingles non mis à jour : {e}");
            }
        });
    }
    #[cfg(not(target_os = "android"))]
    let _ = pins;
}

/// Raccourci touché ou lien `seaglass://pin/<clé>` : la clé vient de l'extérieur, seule une
/// épingle existante est acceptée.
#[cfg(target_os = "android")]
pub fn request(app: &tauri::AppHandle, key: String) {
    use tauri::{Emitter, Manager};

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let pins = super::settings::load(&app.state::<crate::AppState>()).pins;
        let Some(pin) = find(&pins, &key) else {
            tracing::warn!("raccourci d'une épingle absente");
            return;
        };
        if let Ok(mut request) = REQUEST.lock() {
            *request = Some(pin.clone());
        }
        if let Err(e) = app.emit("open-pin", ()) {
            tracing::warn!("épingle non demandée à l'UI : {e}");
        }
    });
}

/// Épingle demandée par un raccourci depuis le dernier appel ; toujours `None` sur le bureau.
/// Lu au démarrage de l'UI et à l'événement `open-pin`.
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
        assert_eq!(list[2].label, "Mix", "titre vide : libellé générique");
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
