//! Traduction des messages JSON de mpv en événements typés.

use serde_json::Value;

use super::device::{self, AudioDevice};
use crate::engine::AudioParams;

/// Propriétés observées (`observe_property`), dans l'ordre des identifiants 1..=N.
pub const OBSERVED: [&str; 11] = [
    "playlist-pos",
    "time-pos",
    "duration",
    "pause",
    "paused-for-cache",
    "audio-params",
    "current-ao",
    "idle-active",
    "audio-device-list",
    "audio-out-params",
    "demuxer-cache-time",
];

#[derive(Debug, Clone, PartialEq)]
pub enum Property {
    PlaylistPos(Option<i64>),
    TimePos(Option<f64>),
    Duration(Option<f64>),
    Pause(bool),
    PausedForCache(bool),
    AudioParams(Option<AudioParams>),
    CurrentAo(Option<String>),
    IdleActive(bool),
    /// Périphériques WASAPI connus de mpv, renotifiés au branchement (#79).
    AudioDeviceList(Vec<AudioDevice>),
    /// Format réellement écrit sur la sortie (#245) : le format de mixage de Windows en
    /// partagé, celui ouvert sur le périphérique en exclusif.
    AudioOutParams(Option<AudioParams>),
    /// Position jusqu'où le démultiplexeur a déjà lu le fichier courant (#453), en
    /// secondes ; un simple nombre, plutôt que l'objet complet `demuxer-cache-state`.
    DemuxerCacheTime(Option<f64>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    Eof,
    Stop,
    Quit,
    Error,
    Redirect,
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MpvEvent {
    Property(Property),
    StartFile,
    FileLoaded,
    EndFile {
        reason: EndReason,
        error: Option<String>,
    },
    /// Le pipe IPC s'est fermé (mpv arrêté ou planté).
    Disconnected,
}

pub fn parse(msg: &Value) -> Option<MpvEvent> {
    match msg.get("event")?.as_str()? {
        "property-change" => parse_property(msg).map(MpvEvent::Property),
        "start-file" => Some(MpvEvent::StartFile),
        "file-loaded" => Some(MpvEvent::FileLoaded),
        "end-file" => Some(MpvEvent::EndFile {
            reason: match msg.get("reason").and_then(Value::as_str) {
                Some("eof") => EndReason::Eof,
                Some("stop") => EndReason::Stop,
                Some("quit") => EndReason::Quit,
                Some("error") => EndReason::Error,
                Some("redirect") => EndReason::Redirect,
                _ => EndReason::Unknown,
            },
            error: msg
                .get("file_error")
                .and_then(Value::as_str)
                .map(str::to_owned),
        }),
        _ => None,
    }
}

fn parse_property(msg: &Value) -> Option<Property> {
    let data = msg.get("data").unwrap_or(&Value::Null);
    let flag = || data.as_bool().unwrap_or(false);
    Some(match msg.get("name")?.as_str()? {
        "playlist-pos" => Property::PlaylistPos(data.as_i64().filter(|p| *p >= 0)),
        "time-pos" => Property::TimePos(data.as_f64()),
        "duration" => Property::Duration(data.as_f64()),
        "pause" => Property::Pause(flag()),
        "paused-for-cache" => Property::PausedForCache(flag()),
        "audio-params" => Property::AudioParams(serde_json::from_value(data.clone()).ok()),
        "current-ao" => Property::CurrentAo(data.as_str().map(str::to_owned)),
        "idle-active" => Property::IdleActive(flag()),
        "audio-device-list" => Property::AudioDeviceList(device::parse_list(data)),
        "audio-out-params" => Property::AudioOutParams(serde_json::from_value(data.clone()).ok()),
        "demuxer-cache-time" => Property::DemuxerCacheTime(data.as_f64()),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn property_changes() {
        let ev = parse(&json!({"event":"property-change","id":2,"name":"time-pos","data":12.5}));
        assert_eq!(ev, Some(MpvEvent::Property(Property::TimePos(Some(12.5)))));

        let ev = parse(&json!({"event":"property-change","id":2,"name":"time-pos"}));
        assert_eq!(ev, Some(MpvEvent::Property(Property::TimePos(None))));

        let ev = parse(&json!({"event":"property-change","id":1,"name":"playlist-pos","data":-1}));
        assert_eq!(ev, Some(MpvEvent::Property(Property::PlaylistPos(None))));

        let ev = parse(
            &json!({"event":"property-change","id":11,"name":"demuxer-cache-time","data":95.2}),
        );
        assert_eq!(
            ev,
            Some(MpvEvent::Property(Property::DemuxerCacheTime(Some(95.2))))
        );
    }

    #[test]
    fn audio_params() {
        let ev = parse(
            &json!({"event":"property-change","name":"audio-params","data":{
                "format":"s32","samplerate":96000,"channels":"stereo","hr-channels":"stereo","channel-count":2
            }}),
        );
        let expected = AudioParams {
            format: "s32".into(),
            samplerate: 96000,
            channel_count: 2,
        };
        assert_eq!(
            ev,
            Some(MpvEvent::Property(Property::AudioParams(Some(expected))))
        );

        // Fin de fichier : mpv envoie la propriété sans `data`.
        let ev = parse(&json!({"event":"property-change","id":6,"name":"audio-params"}));
        assert_eq!(ev, Some(MpvEvent::Property(Property::AudioParams(None))));
    }

    #[test]
    fn end_file_error() {
        let ev = parse(
            &json!({"event":"end-file","reason":"error","playlist_entry_id":1,"file_error":"loading failed"}),
        );
        assert_eq!(
            ev,
            Some(MpvEvent::EndFile {
                reason: EndReason::Error,
                error: Some("loading failed".into())
            })
        );
    }

    #[test]
    fn replies_are_not_events() {
        assert_eq!(parse(&json!({"request_id":3,"error":"success"})), None);
    }
}
