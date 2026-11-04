//! Session média d'Android (#24) : notification, écran verrouillé et touches des écouteurs.
//!
//! Le service Kotlin (`PlaybackService.kt`) publie la session ; ses boutons reviennent ici
//! en `Cmd`, comme les touches média de SMTC sur le bureau. L'acteur lui envoie le titre
//! courant, seulement quand il change, et l'état de l'aléatoire et de la répétition pour
//! les boutons de la notification (#163). Traduction des actions testée sur le bureau.

use serde::Deserialize;

use super::MediaInfo;
use crate::i18n::{Lang, tr};
use crate::player::{Cmd, Repeat};

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
    /// Boutons de la notification (#163) : l'état suivant est décidé ici, d'après le
    /// dernier état envoyé à la session.
    ToggleShuffle,
    CycleRepeat,
    /// Appui sur la notification : l'app s'ouvre sur « En lecture » (#163).
    OpenNowPlaying,
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
            "shuffle" => Self::ToggleShuffle,
            "repeat" => Self::CycleRepeat,
            "openNowPlaying" => Self::OpenNowPlaying,
            other => {
                tracing::warn!("action de la session inconnue : {other}");
                return None;
            }
        })
    }

    /// `None` : l'action ne concerne pas le lecteur (`OpenNowPlaying`).
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    fn into_cmd(self, modes: Modes) -> Option<Cmd> {
        Some(match self {
            Self::Play => Cmd::SetPaused(false),
            Self::Pause => Cmd::SetPaused(true),
            Self::Next => Cmd::Next,
            Self::Previous => Cmd::Previous,
            Self::Seek(secs) => Cmd::Seek(secs),
            Self::ToggleShuffle => Cmd::SetShuffle(!modes.shuffle),
            Self::CycleRepeat => Cmd::SetRepeat(next_repeat(modes.repeat)),
            Self::OpenNowPlaying => return None,
        })
    }
}

/// Aléatoire et répétition tels qu'affichés par la notification, avec la langue de leurs
/// libellés : renvoyés à la session seulement si l'un des trois change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
struct Modes {
    shuffle: bool,
    repeat: Repeat,
    lang: Lang,
}

/// Même cycle que le bouton de la barre de lecture : Off → All → One → Off.
fn next_repeat(repeat: Repeat) -> Repeat {
    match repeat {
        Repeat::Off => Repeat::All,
        Repeat::All => Repeat::One,
        Repeat::One => Repeat::Off,
    }
}

/// Arguments de la commande Kotlin `sessionModes` : état des deux boutons et leurs libellés
/// (lus par TalkBack et affichés par certains écrans).
fn modes_args(modes: Modes) -> serde_json::Value {
    let repeat_label = match modes.repeat {
        Repeat::Off => tr("Répétition désactivée", "Repeat off"),
        Repeat::All => tr("Répéter la file", "Repeat all"),
        Repeat::One => tr("Répéter le titre", "Repeat one"),
    };
    let shuffle_label = if modes.shuffle {
        tr("Lecture aléatoire activée", "Shuffle on")
    } else {
        tr("Lecture aléatoire désactivée", "Shuffle off")
    };
    serde_json::json!({
        "shuffle": modes.shuffle,
        "repeat": modes.repeat,
        "shuffleLabel": shuffle_label,
        "repeatLabel": repeat_label,
    })
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
    use std::sync::{Arc, Mutex};

    use serde::Serialize;
    use tauri::ipc::{Channel, InvokeResponseBody};
    use tauri::{AppHandle, Emitter};
    use tokio::sync::mpsc;

    use super::super::{MediaInfo, NowPlaying};
    use super::{Action, Modes};
    use crate::player::Cmd;

    pub struct MediaSessionHandle {
        /// Dernier titre envoyé : `NowPlaying` change à chaque position.
        last: Mutex<Option<Option<MediaInfo>>>,
        /// Derniers états des boutons envoyés, partagés avec les actions de la session :
        /// un appui sur « aléatoire » inverse l'état affiché.
        modes: Arc<Mutex<Option<Modes>>>,
        _actions: Channel,
    }

    #[derive(Serialize)]
    struct ChannelArgs<'a> {
        channel: &'a Channel,
    }

    /// Branche les actions de la session sur `player`. La commande Kotlin part en tâche :
    /// le branchement se fait pendant `setup`.
    pub fn start(app: &AppHandle, player: mpsc::Sender<Cmd>) -> MediaSessionHandle {
        let modes: Arc<Mutex<Option<Modes>>> = Arc::default();
        let shared = modes.clone();
        let ui = app.clone();
        let channel = Channel::new(move |body| {
            let InvokeResponseBody::Json(json) = body else {
                return Ok(());
            };
            let Some(action) = Action::parse(&json) else {
                return Ok(());
            };
            if action == Action::OpenNowPlaying {
                super::super::request_now_playing();
                if let Err(e) = ui.emit("open-now-playing", ()) {
                    tracing::warn!("« En lecture » non demandé à l'UI : {e}");
                }
                return Ok(());
            }
            let current = shared.lock().ok().and_then(|m| *m).unwrap_or_default();
            if let Some(cmd) = action.into_cmd(current)
                && player.try_send(cmd).is_err()
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
            last: Mutex::new(None),
            modes,
            _actions: channel,
        }
    }

    impl MediaSessionHandle {
        pub fn update(&self, state: NowPlaying) {
            let modes = Modes {
                shuffle: state.shuffle,
                repeat: state.repeat,
                lang: crate::i18n::current(),
            };
            if let Ok(mut last) = self.modes.lock()
                && *last != Some(modes)
            {
                *last = Some(modes);
                send("sessionModes", super::modes_args(modes));
            }
            let Ok(mut last) = self.last.lock() else {
                return;
            };
            if last.as_ref() == Some(&state.media) {
                return;
            }
            let args = super::now_playing_args(state.media.as_ref());
            *last = Some(state.media);
            send("nowPlaying", args);
        }
    }

    /// En JNI direct : la notification se met à jour aussi quand l'app est balayée des
    /// récents, sans activité pour les commandes du plugin (#169).
    fn send(command: &'static str, args: serde_json::Value) {
        tauri::async_runtime::spawn(async move {
            if let Err(e) =
                tauri_plugin_seaglass_mobile::direct::call::<serde::de::IgnoredAny>(command, args)
                    .await
            {
                tracing::warn!("session média non mise à jour ({command}) : {e}");
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
        assert_eq!(
            Action::parse(r#"{"command":"shuffle"}"#),
            Some(Action::ToggleShuffle)
        );
        assert_eq!(
            Action::parse(r#"{"command":"repeat"}"#),
            Some(Action::CycleRepeat)
        );
        assert_eq!(
            Action::parse(r#"{"command":"openNowPlaying"}"#),
            Some(Action::OpenNowPlaying)
        );
        assert_eq!(Action::parse(r#"{"command":"seek"}"#), None);
        assert_eq!(Action::parse(r#"{"command":"rewind"}"#), None);
        assert_eq!(Action::parse("pas du json"), None);
    }

    #[test]
    fn repeat_cycles_like_the_player_bar() {
        assert_eq!(next_repeat(Repeat::Off), Repeat::All);
        assert_eq!(next_repeat(Repeat::All), Repeat::One);
        assert_eq!(next_repeat(Repeat::One), Repeat::Off);
    }

    #[test]
    fn modes_carry_state_and_labels() {
        let modes = Modes {
            shuffle: true,
            repeat: Repeat::One,
            lang: Lang::Fr,
        };
        assert_eq!(
            modes_args(modes),
            serde_json::json!({
                "shuffle": true,
                "repeat": "one",
                "shuffleLabel": "Lecture aléatoire activée",
                "repeatLabel": "Répéter le titre",
            })
        );
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
