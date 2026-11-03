//! Session média d'Android (#24) : notification, écran verrouillé et touches des écouteurs.
//!
//! Le service Kotlin (`PlaybackService.kt`) publie la session ; ses boutons reviennent ici
//! en `Cmd`, comme les touches média de SMTC sur le bureau. L'acteur lui envoie le titre
//! courant, seulement quand il change. Traduction des actions testée sur le bureau.

use serde::Deserialize;

use super::MediaInfo;
use crate::player::Cmd;

/// Action de la session, envoyée par `Playback.command`.
#[derive(Deserialize)]
struct Raw {
    command: String,
    /// Secondes, pour `seek`.
    position: Option<f64>,
}

/// Action reconnue. Pas directement un `Cmd` : en détruire un dans un test tirerait le code
/// de la zone de notification, et le binaire de test Windows ne démarre plus (comctl32 v6).
#[derive(Debug, PartialEq)]
enum Action {
    Play,
    Pause,
    Next,
    Previous,
    Seek(f64),
}

impl Action {
    fn parse(json: &str) -> Option<Self> {
        let raw: Raw = serde_json::from_str(json)
            .inspect_err(|e| tracing::warn!("action de la session illisible : {e}"))
            .ok()?;
        Some(match raw.command.as_str() {
            "play" => Self::Play,
            "pause" => Self::Pause,
            "next" => Self::Next,
            "previous" => Self::Previous,
            "seek" => Self::Seek(raw.position?.max(0.0)),
            other => {
                tracing::warn!("action de la session inconnue : {other}");
                return None;
            }
        })
    }

    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    fn into_cmd(self) -> Cmd {
        match self {
            Self::Play => Cmd::SetPaused(false),
            Self::Pause => Cmd::SetPaused(true),
            Self::Next => Cmd::Next,
            Self::Previous => Cmd::Previous,
            Self::Seek(secs) => Cmd::Seek(secs),
        }
    }
}

/// Arguments de la commande Kotlin `nowPlaying` (rien = notification vide).
fn now_playing_args(media: Option<&MediaInfo>) -> serde_json::Value {
    match media {
        Some(m) => serde_json::json!({
            "title": m.title,
            "artist": m.artist,
            "album": m.album,
            "artworkUrl": m.cover_url,
        }),
        None => serde_json::json!({}),
    }
}

#[cfg(target_os = "android")]
pub use android::{MediaSessionHandle, start};

#[cfg(target_os = "android")]
mod android {
    use std::sync::Mutex;

    use serde::Serialize;
    use tauri::ipc::{Channel, InvokeResponseBody};
    use tauri::{AppHandle, Runtime};
    use tokio::sync::mpsc;

    use super::super::{MediaInfo, NowPlaying};
    use crate::player::Cmd;

    pub struct MediaSessionHandle {
        app: AppHandle,
        /// Dernier titre envoyé : `NowPlaying` change à chaque position.
        last: Mutex<Option<Option<MediaInfo>>>,
        _actions: Channel,
    }

    #[derive(Serialize)]
    struct ChannelArgs<'a> {
        channel: &'a Channel,
    }

    /// Branche les actions de la session sur `player`. La commande Kotlin part en tâche :
    /// le branchement se fait pendant `setup`.
    pub fn start(app: &AppHandle, player: mpsc::Sender<Cmd>) -> MediaSessionHandle {
        let channel = Channel::new(move |body| {
            if let InvokeResponseBody::Json(json) = body
                && let Some(action) = super::Action::parse(&json)
                && player.try_send(action.into_cmd()).is_err()
            {
                tracing::warn!("action de la session perdue : lecteur occupé");
            }
            Ok(())
        });
        let handle = app.clone();
        let args = channel.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = tauri_plugin_seaglass_mobile::native(&handle)
                .call("sessionStart", ChannelArgs { channel: &args })
                .await
            {
                tracing::warn!("session média indisponible : {e}");
            }
        });
        MediaSessionHandle {
            app: app.clone(),
            last: Mutex::new(None),
            _actions: channel,
        }
    }

    impl MediaSessionHandle {
        pub fn update(&self, state: NowPlaying) {
            let Ok(mut last) = self.last.lock() else {
                return;
            };
            if last.as_ref() == Some(&state.media) {
                return;
            }
            let args = super::now_playing_args(state.media.as_ref());
            *last = Some(state.media);
            send(self.app.clone(), args);
        }
    }

    fn send<R: Runtime>(app: AppHandle<R>, args: serde_json::Value) {
        tauri::async_runtime::spawn(async move {
            if let Err(e) = tauri_plugin_seaglass_mobile::native(&app)
                .call("nowPlaying", args)
                .await
            {
                tracing::warn!("titre de la session non mis à jour : {e}");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_actions_are_recognized() {
        assert_eq!(Action::parse(r#"{"command":"play"}"#), Some(Action::Play));
        assert_eq!(Action::parse(r#"{"command":"pause"}"#), Some(Action::Pause));
        assert_eq!(Action::parse(r#"{"command":"next"}"#), Some(Action::Next));
        assert_eq!(
            Action::parse(r#"{"command":"previous"}"#),
            Some(Action::Previous)
        );
        assert_eq!(
            Action::parse(r#"{"command":"seek","position":61.5}"#),
            Some(Action::Seek(61.5))
        );
        assert_eq!(
            Action::parse(r#"{"command":"seek","position":-2}"#),
            Some(Action::Seek(0.0))
        );
        assert_eq!(Action::parse(r#"{"command":"seek"}"#), None);
        assert_eq!(Action::parse(r#"{"command":"rewind"}"#), None);
        assert_eq!(Action::parse("pas du json"), None);
    }

    #[test]
    fn now_playing_carries_the_cover() {
        let media = MediaInfo {
            title: "Time".into(),
            artist: "Pink Floyd".into(),
            album: "The Dark Side of the Moon".into(),
            cover_url: Some("https://resources.tidal.com/images/a/b/640x640.jpg".into()),
            duration: 413.0,
        };
        assert_eq!(
            now_playing_args(Some(&media)),
            serde_json::json!({
                "title": "Time",
                "artist": "Pink Floyd",
                "album": "The Dark Side of the Moon",
                "artworkUrl": "https://resources.tidal.com/images/a/b/640x640.jpg",
            })
        );
        assert_eq!(now_playing_args(None), serde_json::json!({}));
    }
}
