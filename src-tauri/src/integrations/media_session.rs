//! Android media session (#24): notification, lock screen and headphone buttons.
//!
//! The Kotlin service (`PlaybackService.kt`) publishes the session; its buttons come back here as
//! `Cmd`s, like the SMTC media keys on the desktop. The actor sends it the current track, only when
//! it changes, and the shuffle and repeat state for the notification's buttons (#163). The
//! translation of the actions is tested on the desktop.

use serde::Deserialize;

use super::MediaInfo;
use crate::i18n::{Lang, tr};
use crate::player::{Cmd, Repeat};

/// Session action, sent by `Playback.command`.
#[derive(Deserialize)]
struct Raw {
    command: String,
    /// Seconds, for `seek`.
    position: Option<f64>,
    /// Shared text, for `openLink` (#203).
    text: Option<String>,
    /// Pin key, for `openPin` (#335).
    pin: Option<String>,
    /// State requested by a controller, for `setRepeat` and `setShuffle` (#254).
    repeat: Option<Repeat>,
    shuffle: Option<bool>,
}

/// Recognized action. Not a `Cmd` directly: dropping one in a test would pull in the notification
/// area code, and the Windows test binary no longer starts (comctl32 v6).
#[derive(Debug, PartialEq)]
enum Action {
    Play,
    Pause,
    Next,
    Previous,
    Seek(f64),
    /// Notification buttons (#163): the next state is decided here, from the last state sent to the
    /// session.
    ToggleShuffle,
    CycleRepeat,
    /// Standard commands of a controller (car radio, watch…): the requested state (#254).
    SetShuffle(bool),
    SetRepeat(Repeat),
    /// Tap on the notification: the app opens on "Now playing" (#163).
    OpenNowPlaying,
    /// `seaglass://drive` link or app shortcut: driving mode (#193).
    OpenDrive,
    /// Text received through Android's "Share" menu (#203).
    OpenLink(String),
    /// Shortcut of a pin or `seaglass://pin/<key>` link (#335).
    OpenPin(String),
}

impl Action {
    fn parse(json: &str) -> Option<Self> {
        let raw: Raw = serde_json::from_str(json)
            .inspect_err(|e| tracing::warn!("unreadable session action: {e}"))
            .ok()?;
        Some(match raw.command.as_str() {
            "play" => Self::Play,
            "pause" => Self::Pause,
            "next" => Self::Next,
            "previous" => Self::Previous,
            "seek" => Self::Seek(raw.position?.max(0.0)),
            "shuffle" => Self::ToggleShuffle,
            "repeat" => Self::CycleRepeat,
            "setShuffle" => Self::SetShuffle(raw.shuffle?),
            "setRepeat" => Self::SetRepeat(raw.repeat?),
            "openNowPlaying" => Self::OpenNowPlaying,
            "openDrive" => Self::OpenDrive,
            "openLink" => Self::OpenLink(raw.text?),
            "openPin" => Self::OpenPin(raw.pin?),
            other => {
                tracing::warn!("unknown session action: {other}");
                return None;
            }
        })
    }

    /// `None`: the action does not concern the player (`OpenNowPlaying`, `OpenDrive`…).
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
            Self::SetShuffle(on) => Cmd::SetShuffle(on),
            Self::SetRepeat(repeat) => Cmd::SetRepeat(repeat),
            Self::OpenNowPlaying | Self::OpenDrive | Self::OpenLink(_) | Self::OpenPin(_) => {
                return None;
            }
        })
    }
}

/// Shuffle and repeat as the notification shows them, with the language of their labels: sent back
/// to the session only if one of the three changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
struct Modes {
    shuffle: bool,
    repeat: Repeat,
    lang: Lang,
}

/// Same cycle as the playback bar's button: Off → All → One → Off.
fn next_repeat(repeat: Repeat) -> Repeat {
    match repeat {
        Repeat::Off => Repeat::All,
        Repeat::All => Repeat::One,
        Repeat::One => Repeat::Off,
    }
}

/// Arguments of the Kotlin `sessionModes` command: the state of both buttons and their labels (read
/// by TalkBack and shown by some screens).
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

/// Arguments of the Kotlin `nowPlaying` command (nothing = empty notification).
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
        /// Last track sent: `NowPlaying` changes with every position.
        last: Mutex<Option<Option<MediaInfo>>>,
        /// Last button states sent, shared with the session's actions: a tap on "shuffle" flips the
        /// state shown.
        modes: Arc<Mutex<Option<Modes>>>,
        _actions: Channel,
    }

    #[derive(Serialize)]
    struct ChannelArgs<'a> {
        channel: &'a Channel,
    }

    /// Attaches the session's actions to `player`. The Kotlin command runs as a task: the attaching
    /// happens during `setup`.
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
            if let Action::OpenLink(text) = action {
                crate::commands::link::share(&ui, text);
                return Ok(());
            }
            if let Action::OpenPin(key) = action {
                crate::commands::shortcuts::request(&ui, key);
                return Ok(());
            }
            if action == Action::OpenNowPlaying {
                super::super::request_now_playing();
                if let Err(e) = ui.emit("open-now-playing", ()) {
                    tracing::warn!("\"Now playing\" not requested from the UI: {e}");
                }
                return Ok(());
            }
            if action == Action::OpenDrive {
                super::super::request_drive();
                if let Err(e) = ui.emit("open-drive", ()) {
                    tracing::warn!("driving mode not requested from the UI: {e}");
                }
                return Ok(());
            }
            let current = shared.lock().ok().and_then(|m| *m).unwrap_or_default();
            if let Some(cmd) = action.into_cmd(current)
                && player.try_send(cmd).is_err()
            {
                tracing::warn!("session action lost: player busy");
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
                tracing::warn!("media session unavailable: {e}");
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

    /// Through JNI directly: the notification is also updated when the app is swiped from the
    /// recents, with no activity for the plugin's commands (#169).
    fn send(command: &'static str, args: serde_json::Value) {
        tauri::async_runtime::spawn(async move {
            if let Err(e) =
                tauri_plugin_seaglass_mobile::direct::call::<serde::de::IgnoredAny>(command, args)
                    .await
            {
                tracing::warn!("media session not updated ({command}): {e}");
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
            Action::parse(r#"{"command":"setShuffle","shuffle":true}"#),
            Some(Action::SetShuffle(true))
        );
        assert_eq!(
            Action::parse(r#"{"command":"setRepeat","repeat":"all"}"#),
            Some(Action::SetRepeat(Repeat::All))
        );
        assert_eq!(Action::parse(r#"{"command":"setShuffle"}"#), None);
        assert_eq!(
            Action::parse(r#"{"command":"setRepeat","repeat":"twice"}"#),
            None
        );
        assert_eq!(
            Action::parse(r#"{"command":"openNowPlaying"}"#),
            Some(Action::OpenNowPlaying)
        );
        assert_eq!(
            Action::parse(r#"{"command":"openPin","pin":"later"}"#),
            Some(Action::OpenPin("later".into()))
        );
        assert_eq!(Action::parse(r#"{"command":"openPin"}"#), None);
        assert_eq!(
            Action::parse(r#"{"command":"openDrive"}"#),
            Some(Action::OpenDrive)
        );
        assert_eq!(
            Action::parse(r#"{"command":"openLink","text":"https://tidal.com/album/1"}"#),
            Some(Action::OpenLink("https://tidal.com/album/1".into()))
        );
        assert_eq!(Action::parse(r#"{"command":"openLink"}"#), None);
        assert_eq!(Action::parse(r#"{"command":"seek"}"#), None);
        assert_eq!(Action::parse(r#"{"command":"rewind"}"#), None);
        assert_eq!(Action::parse("not json"), None);
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
            track_id: 1,
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
