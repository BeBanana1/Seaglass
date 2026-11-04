//! Moteur Media3 (Android, #24) : traduit `engine::Op` en commandes du plugin Kotlin
//! (`Playback.kt`) et ses événements JSON en `engine::Event`. Logique pure testée (aussi
//! sur le bureau) ; seul `Media3Engine` n'existe que sur Android.
//!
//! Sur Android, le MPD est passé à Media3 en URL `data:` : aucun serveur local, donc rien
//! de joignable par les autres apps du téléphone.

// Hors Android, seuls les tests utilisent ce module.
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use base64::Engine as _;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::engine::{AudioParams, End, Event, Op};

/// MPD en URL `data:` pour Media3.
pub fn data_url(mpd: &str) -> String {
    format!(
        "data:application/dash+xml;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(mpd)
    )
}

/// Arguments de la commande Kotlin `engine` ; `None` pour une opération sans objet sur
/// Android (cache de mpv, périphérique de sortie).
pub fn op_args(op: &Op) -> Option<Value> {
    Some(match op {
        Op::Play { url, start, gain } => {
            json!({"op": "play", "url": url, "start": start, "gain": gain})
        }
        Op::Preload { url, gain } => json!({"op": "preload", "url": url, "gain": gain}),
        Op::RemoveNext => json!({"op": "removeNext"}),
        Op::DropPrevious => json!({"op": "dropPrevious"}),
        Op::Stop => json!({"op": "stop"}),
        Op::TogglePause => json!({"op": "togglePause"}),
        Op::SetPaused(paused) => json!({"op": "setPaused", "paused": paused}),
        Op::Seek(secs) => json!({"op": "seek", "position": secs}),
        Op::LoopCurrent(on) => json!({"op": "loop", "on": on}),
        Op::Volume { level, muted } => json!({"op": "volume", "level": level, "muted": muted}),
        Op::CurrentGain(gain) => json!({"op": "currentGain", "gain": gain}),
        Op::CacheSize(_) | Op::OutputDevice(_) => return None,
    })
}

/// Événement envoyé par `Playback.kt` (champ `type`).
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Native {
    Position {
        value: Option<f64>,
    },
    Duration {
        value: Option<f64>,
    },
    Paused {
        value: bool,
    },
    Buffering {
        value: bool,
    },
    Idle {
        value: bool,
    },
    Driver {
        value: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Format {
        format: String,
        samplerate: u32,
        channel_count: u32,
    },
    Loaded,
    Advanced,
    Ended {
        reason: String,
        error: Option<String>,
    },
}

/// Événement du moteur ; `None` s'il est illisible (journalisé) ou sans intérêt.
pub fn translate(json: &str) -> Option<Event> {
    let native = match serde_json::from_str::<Native>(json) {
        Ok(native) => native,
        Err(e) => {
            tracing::warn!("événement Media3 illisible : {e}");
            return None;
        }
    };
    Some(match native {
        Native::Position { value } => Event::Position(value),
        Native::Duration { value } => Event::Duration(value),
        Native::Paused { value } => Event::Paused(value),
        Native::Buffering { value } => Event::Buffering(value),
        Native::Idle { value } => Event::Idle(value),
        Native::Driver { value } => Event::Driver(value),
        Native::Format {
            format,
            samplerate,
            channel_count,
        } => Event::Format(Some(AudioParams {
            format,
            samplerate,
            channel_count,
        })),
        Native::Loaded => Event::Loaded,
        Native::Advanced => Event::Advanced,
        Native::Ended { reason, error } => Event::Ended(match reason.as_str() {
            "eof" => End::Eof,
            _ => End::Error(error),
        }),
    })
}

#[cfg(target_os = "android")]
pub use android::{Launch, Media3Engine};

#[cfg(target_os = "android")]
mod android {
    use serde::Serialize;
    use tauri::AppHandle;
    use tauri::ipc::{Channel, InvokeResponseBody};
    use tokio::sync::mpsc;

    use crate::engine::{Engine, Event, Op};
    use crate::mpv::CacheSize;

    pub struct Media3Engine {
        /// Gardé pour la durée du moteur : les événements Kotlin y arrivent.
        _events: Channel,
    }

    #[derive(Serialize)]
    struct ChannelArgs<'a> {
        channel: &'a Channel,
    }

    pub struct Launch {
        pub app: AppHandle,
    }

    impl Launch {
        /// Crée le lecteur Kotlin et branche ses événements sur `events`. Le cache et le
        /// périphérique de sortie sont des réglages du bureau.
        pub async fn start(
            &self,
            _cache: CacheSize,
            _audio_device: Option<&str>,
            events: mpsc::Sender<Event>,
        ) -> Result<Media3Engine, String> {
            // Le canal est rappelé sur le thread principal d'Android, qui ne doit jamais
            // attendre l'acteur (l'acteur peut attendre ce thread) : file non bornée,
            // vidée par une tâche.
            let (raw_tx, mut raw_rx) = mpsc::unbounded_channel();
            tauri::async_runtime::spawn(async move {
                while let Some(ev) = raw_rx.recv().await {
                    if events.send(ev).await.is_err() {
                        break;
                    }
                }
            });
            let channel = Channel::new(move |body| {
                if let InvokeResponseBody::Json(json) = body
                    && let Some(ev) = super::translate(&json)
                {
                    let _ = raw_tx.send(ev);
                }
                Ok(())
            });
            tauri_plugin_seaglass_mobile::native(&self.app)
                .call("engineStart", ChannelArgs { channel: &channel })
                .await?;
            Ok(Media3Engine { _events: channel })
        }
    }

    impl Engine for Media3Engine {
        async fn apply(&self, op: Op) -> Result<(), String> {
            let Some(args) = super::op_args(&op) else {
                return Ok(());
            };
            // En JNI direct : la lecture continue quand l'app est balayée des récents,
            // sans activité pour les commandes du plugin (#169).
            tauri_plugin_seaglass_mobile::direct::call::<serde::de::IgnoredAny>("engine", args)
                .await
                .map(drop)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops_follow_the_kotlin_arguments() {
        let play = Op::Play {
            url: "data:x".into(),
            start: 12.5,
            gain: Some(-3.0),
        };
        assert_eq!(
            op_args(&play),
            Some(json!({"op": "play", "url": "data:x", "start": 12.5, "gain": -3.0}))
        );
        assert_eq!(
            op_args(&Op::Preload {
                url: "u".into(),
                gain: None
            }),
            Some(json!({"op": "preload", "url": "u", "gain": null}))
        );
        assert_eq!(
            op_args(&Op::Seek(42.0)),
            Some(json!({"op": "seek", "position": 42.0}))
        );
        assert_eq!(
            op_args(&Op::Volume {
                level: 60,
                muted: false
            }),
            Some(json!({"op": "volume", "level": 60, "muted": false}))
        );
        assert_eq!(
            op_args(&Op::LoopCurrent(true)),
            Some(json!({"op": "loop", "on": true}))
        );
        assert_eq!(op_args(&Op::OutputDevice("auto".into())), None);
    }

    #[test]
    fn native_events_are_translated() {
        assert_eq!(
            translate(r#"{"type":"position","value":1.25}"#),
            Some(Event::Position(Some(1.25)))
        );
        assert_eq!(
            translate(r#"{"type":"position"}"#),
            Some(Event::Position(None))
        );
        assert_eq!(
            translate(r#"{"type":"paused","value":true}"#),
            Some(Event::Paused(true))
        );
        assert_eq!(translate(r#"{"type":"advanced"}"#), Some(Event::Advanced));
        assert_eq!(
            translate(r#"{"type":"format","format":"s16","samplerate":44100,"channelCount":2}"#),
            Some(Event::Format(Some(AudioParams {
                format: "s16".into(),
                samplerate: 44_100,
                channel_count: 2,
            })))
        );
        assert_eq!(
            translate(r#"{"type":"ended","reason":"eof"}"#),
            Some(Event::Ended(End::Eof))
        );
        assert_eq!(
            translate(
                r#"{"type":"ended","reason":"error","error":"ERROR_CODE_IO_BAD_HTTP_STATUS"}"#
            ),
            Some(Event::Ended(End::Error(Some(
                "ERROR_CODE_IO_BAD_HTTP_STATUS".into()
            ))))
        );
        assert_eq!(translate(r#"{"type":"unknown"}"#), None);
    }

    #[test]
    fn mpd_goes_in_a_data_url() {
        assert_eq!(
            data_url("<MPD/>"),
            "data:application/dash+xml;base64,PE1QRC8+"
        );
    }
}
