//! Play log (#34): each listen reported to TIDAL the way the official clients do, so that
//! it is counted. Format read in the official open-source SDKs (`tidal-sdk-web`
//! `player/src/internal/event-tracking/play-log/` and `event-producer/`,
//! `tidal-sdk-android` `eventproducer/`), checked with `spike/playlog.py`; notes in
//! `docs/tidal.md` ("Play log").
//!
//! A `playback_session` event per listen goes to the event collector in an SQS
//! `SendMessageBatch` form. The event body holds no token: it is queued as is and the
//! token is only added at send time, in the `Authorization` header and in the `Headers`
//! attribute. Never log the form.

use serde_json::json;

use super::api::TidalClient;
use super::models::{LogAction, LogSource, PlayLog};
use super::{Result, TidalError};

const EVENT_BATCH_URL: &str = "https://ec.tidal.com/api/event-batch";
/// Most entries an SQS batch accepts.
pub const MAX_BATCH: usize = 10;

#[cfg(windows)]
const OS_NAME: &str = "Windows";
#[cfg(target_os = "android")]
const OS_NAME: &str = "Android";
#[cfg(all(not(windows), not(target_os = "android")))]
const OS_NAME: &str = "Linux";

/// Random (version 4) UUID, for a streaming session or an event.
pub fn new_uuid() -> String {
    let mut b: [u8; 16] = rand::random();
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// Event body (`MessageBody`) of `log`, with its event `uuid` and creation time `ts`
/// (Unix ms), both fixed when it is queued so that a resend is the same event.
pub fn message_body(log: &PlayLog, uuid: &str, ts: i64) -> String {
    let s = &log.session;
    let (source_type, source_id) = match &log.source {
        LogSource::Album(id) => ("ALBUM", id.to_string()),
        LogSource::Playlist(uuid) => ("PLAYLIST", uuid.clone()),
        LogSource::Mix(id) => ("MIX", id.clone()),
        LogSource::Artist(id) => ("ARTIST", id.to_string()),
    };
    json!({
        "group": "play_log",
        "name": "playback_session",
        "version": 2,
        "ts": ts,
        "uuid": uuid,
        "payload": {
            "actions": log.actions.iter().map(action).collect::<Vec<_>>(),
            "actualAssetPresentation": s.presentation,
            "actualAudioMode": s.audio_mode,
            "actualProductId": s.served.to_string(),
            "actualQuality": s.quality.as_api_str(),
            "endAssetPosition": seconds(log.end_position),
            "endTimestamp": log.end_timestamp,
            "isPostPaywall": s.presentation == "FULL",
            "playbackSessionId": s.id,
            "productType": "TRACK",
            "requestedProductId": s.requested.to_string(),
            "sourceId": source_id,
            "sourceType": source_type,
            "startAssetPosition": seconds(log.start_position),
            "startTimestamp": log.start_timestamp,
        },
    })
    .to_string()
}

fn action(a: &LogAction) -> serde_json::Value {
    json!({
        "actionType": if a.start { "PLAYBACK_START" } else { "PLAYBACK_STOP" },
        "assetPosition": seconds(a.position),
        "timestamp": a.timestamp,
    })
}

/// Position rounded to the millisecond, never negative.
fn seconds(pos: f64) -> f64 {
    (pos.max(0.0) * 1000.0).round() / 1000.0
}

/// A queued event: its uuid (the SQS entry id) and its body.
pub struct Outgoing<'a> {
    pub uuid: &'a str,
    pub body: &'a str,
}

/// What the collector did with a batch: entries accepted, entries refused as malformed
/// (`SenderFault`, never accepted on a resend). The others are to be sent again.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct BatchOutcome {
    pub accepted: Vec<String>,
    pub malformed: Vec<String>,
}

/// `SendMessageBatch` form; `headers` is the JSON of the `Headers` attribute.
fn batch_form(events: &[Outgoing<'_>], headers: &str) -> Vec<(String, String)> {
    let mut form = Vec::with_capacity(events.len() * 8);
    for (i, event) in events.iter().enumerate() {
        let entry = format!("SendMessageBatchRequestEntry.{}", i + 1);
        let attr = format!("{entry}.MessageAttribute");
        form.extend([
            (format!("{entry}.Id"), event.uuid.to_owned()),
            (format!("{entry}.MessageBody"), event.body.to_owned()),
            (format!("{attr}.1.Name"), "Name".to_owned()),
            (
                format!("{attr}.1.Value.StringValue"),
                "playback_session".to_owned(),
            ),
            (format!("{attr}.1.Value.DataType"), "String".to_owned()),
            (format!("{attr}.2.Name"), "Headers".to_owned()),
            (format!("{attr}.2.Value.DataType"), "String".to_owned()),
            (format!("{attr}.2.Value.StringValue"), headers.to_owned()),
        ]);
    }
    form
}

/// `Headers` attribute, as the official clients send it (the token too).
fn headers_attribute(client_id: &str, token: &str, now: i64) -> String {
    json!({
        "app-name": "Seaglass",
        "app-version": env!("CARGO_PKG_VERSION"),
        "client-id": client_id,
        "consent-category": "NECESSARY",
        "os-name": OS_NAME,
        "requested-sent-timestamp": now.to_string(),
        "authorization": token,
    })
    .to_string()
}

/// Reads the collector's SQS XML answer.
fn parse_outcome(xml: &str) -> BatchOutcome {
    let mut outcome = BatchOutcome::default();
    for entry in blocks(xml, "SendMessageBatchResultEntry") {
        if let Some(id) = blocks(entry, "Id").next() {
            outcome.accepted.push(id.trim().to_owned());
        }
    }
    for entry in blocks(xml, "BatchResultErrorEntry") {
        let sender_fault = blocks(entry, "SenderFault").next().map(str::trim) == Some("true");
        if sender_fault && let Some(id) = blocks(entry, "Id").next() {
            outcome.malformed.push(id.trim().to_owned());
        }
    }
    outcome
}

/// Contents of each `<tag>…</tag>` of `xml`, in order (no nesting of the same tag).
fn blocks<'a>(xml: &'a str, tag: &str) -> impl Iterator<Item = &'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut rest = xml;
    std::iter::from_fn(move || {
        let start = rest.find(&open)? + open.len();
        let len = rest[start..].find(&close)?;
        let inner = &rest[start..start + len];
        rest = &rest[start + len + close.len()..];
        Some(inner)
    })
}

impl TidalClient {
    /// Posts up to `MAX_BATCH` queued events under the current session's token,
    /// refreshing it once on a 401. `now`: send time (Unix ms).
    pub async fn send_play_logs(&self, events: &[Outgoing<'_>], now: i64) -> Result<BatchOutcome> {
        let mut rejected: Option<String> = None;
        loop {
            let (token, _) = self.credentials(rejected.as_deref()).await?;
            let headers = headers_attribute(&self.creds.id, &token, now);
            let resp = self
                .http
                .post(EVENT_BATCH_URL)
                .bearer_auth(&token)
                .form(&batch_form(events, &headers))
                .send()
                .await?;
            let status = resp.status();
            if status.as_u16() == 401 && rejected.is_none() {
                rejected = Some(token);
                continue;
            }
            let text = resp.text().await?;
            if !status.is_success() {
                return Err(TidalError::Api {
                    status: status.as_u16(),
                    message: "event-batch".to_owned(),
                });
            }
            return Ok(parse_outcome(&text));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::{AudioQuality, PlaybackSession, TrackId};

    fn log() -> PlayLog {
        PlayLog {
            session: PlaybackSession {
                id: "1877d2dd-552f-408d-9540-14096b6a2754".to_owned(),
                requested: TrackId(77599598),
                served: TrackId(77599598),
                presentation: "FULL".to_owned(),
                audio_mode: Some("STEREO".to_owned()),
                quality: AudioQuality::Lossless,
            },
            source: LogSource::Album(77599591),
            start_timestamp: 1_795_589_914_127,
            start_position: 0.0,
            end_timestamp: 1_795_589_958_316,
            end_position: 44.0,
            actions: vec![
                LogAction {
                    start: false,
                    position: 10.123_456,
                    timestamp: 1_795_589_924_000,
                },
                LogAction {
                    start: true,
                    position: 10.123_456,
                    timestamp: 1_795_589_925_000,
                },
            ],
        }
    }

    #[test]
    fn body_matches_the_spike_event() {
        let body: serde_json::Value =
            serde_json::from_str(&message_body(&log(), "ev-1", 1_795_590_260_133)).unwrap();
        assert_eq!(body["group"], "play_log");
        assert_eq!(body["name"], "playback_session");
        assert_eq!(body["version"], 2);
        assert_eq!(body["uuid"], "ev-1");
        assert_eq!(body["ts"], 1_795_590_260_133_i64);
        let p = &body["payload"];
        assert_eq!(p["actualProductId"], "77599598");
        assert_eq!(p["requestedProductId"], "77599598");
        assert_eq!(p["actualQuality"], "LOSSLESS");
        assert_eq!(p["actualAudioMode"], "STEREO");
        assert_eq!(p["actualAssetPresentation"], "FULL");
        assert_eq!(p["isPostPaywall"], true);
        assert_eq!(p["productType"], "TRACK");
        assert_eq!(p["sourceType"], "ALBUM");
        assert_eq!(p["sourceId"], "77599591");
        assert_eq!(p["startAssetPosition"], 0.0);
        assert_eq!(p["endAssetPosition"], 44.0);
        assert_eq!(
            p["playbackSessionId"],
            "1877d2dd-552f-408d-9540-14096b6a2754"
        );
        assert_eq!(p["actions"][0]["actionType"], "PLAYBACK_STOP");
        assert_eq!(p["actions"][0]["assetPosition"], 10.123);
        assert_eq!(p["actions"][1]["actionType"], "PLAYBACK_START");
        // Nothing that is not in the official event.
        assert_eq!(p.as_object().unwrap().len(), 15);
        assert_eq!(body.as_object().unwrap().len(), 6);
    }

    #[test]
    fn form_carries_each_event_and_the_headers() {
        let events = [
            Outgoing {
                uuid: "a",
                body: "{\"n\":1}",
            },
            Outgoing {
                uuid: "b",
                body: "{\"n\":2}",
            },
        ];
        let form = batch_form(&events, "{\"h\":1}");
        let get = |k: &str| {
            form.iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(get("SendMessageBatchRequestEntry.1.Id"), Some("a"));
        assert_eq!(
            get("SendMessageBatchRequestEntry.2.MessageBody"),
            Some("{\"n\":2}")
        );
        assert_eq!(
            get("SendMessageBatchRequestEntry.2.MessageAttribute.1.Value.StringValue"),
            Some("playback_session")
        );
        assert_eq!(
            get("SendMessageBatchRequestEntry.1.MessageAttribute.2.Value.StringValue"),
            Some("{\"h\":1}")
        );
        assert_eq!(form.len(), 16);
    }

    #[test]
    fn headers_hold_what_the_sdk_sends() {
        let h: serde_json::Value =
            serde_json::from_str(&headers_attribute("cid", "tok", 42)).unwrap();
        assert_eq!(h["consent-category"], "NECESSARY");
        assert_eq!(h["client-id"], "cid");
        assert_eq!(h["authorization"], "tok");
        assert_eq!(h["requested-sent-timestamp"], "42");
        assert_eq!(h["app-name"], "Seaglass");
    }

    #[test]
    fn outcome_of_the_spike_answer() {
        let xml = r#"<?xml version="1.0"?><SendMessageBatchResponse xmlns="http://queue.amazonaws.com/doc/2012-11-05/"><SendMessageBatchResult><SendMessageBatchResultEntry><Id>be4d1798</Id><MessageId>c68f</MessageId></SendMessageBatchResultEntry><BatchResultErrorEntry><Id>bad</Id><SenderFault>true</SenderFault><Code>x</Code></BatchResultErrorEntry><BatchResultErrorEntry><Id>later</Id><SenderFault>false</SenderFault></BatchResultErrorEntry></SendMessageBatchResult></SendMessageBatchResponse>"#;
        assert_eq!(
            parse_outcome(xml),
            BatchOutcome {
                accepted: vec!["be4d1798".to_owned()],
                malformed: vec!["bad".to_owned()],
            }
        );
        assert_eq!(parse_outcome("garbage"), BatchOutcome::default());
    }

    #[test]
    fn uuids_are_version_4() {
        let id = new_uuid();
        assert_eq!(id.len(), 36);
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(
            parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
            [8, 4, 4, 4, 12]
        );
        assert!(parts[2].starts_with('4'));
        assert!(matches!(&parts[3][..1], "8" | "9" | "a" | "b"));
        assert_ne!(new_uuid(), id);
    }
}
