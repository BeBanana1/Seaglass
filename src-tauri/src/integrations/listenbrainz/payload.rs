//! Listens in the format of the ListenBrainz API (`submit-listens`), pure logic.
//!
//! `additional_info` follows the keys ListenBrainz recommends: the ISRC and the duration help the
//! match with MusicBrainz, `music_service` is the service's canonical domain.

use serde_json::{Value, json};

use crate::tidal::models::Track;

const PLAYER: &str = "Seaglass";
const SERVICE: &str = "tidal.com";
/// ListenBrainz refuses a listen of more than 10,240 bytes: the free fields are cut well before
/// that (500 characters, 2,000 bytes at worst).
const MAX_FIELD_CHARS: usize = 500;

/// Submission type: start of a track, one listen, or several stored listens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListenType {
    PlayingNow,
    Single,
    Import,
}

impl ListenType {
    fn as_str(self) -> &'static str {
        match self {
            Self::PlayingNow => "playing_now",
            Self::Single => "single",
            Self::Import => "import",
        }
    }
}

/// One listen: `listened_at` (Unix seconds, start of the listen) is absent for `playing_now`.
pub fn listen(track: &Track, listened_at: Option<i64>) -> Value {
    let title = match &track.version {
        Some(v) if !v.trim().is_empty() => format!("{} ({})", track.title, v.trim()),
        _ => track.title.clone(),
    };
    let artist = track
        .artists
        .iter()
        .map(|a| a.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let mut info = json!({
        "media_player": PLAYER,
        "media_player_version": env!("CARGO_PKG_VERSION"),
        "submission_client": PLAYER,
        "submission_client_version": env!("CARGO_PKG_VERSION"),
        "music_service": SERVICE,
        "origin_url": format!("https://{SERVICE}/track/{}", track.id.0),
    });
    if track.duration > 0 {
        info["duration_ms"] = json!(u64::from(track.duration) * 1000);
    }
    if let Some(isrc) = track.isrc.as_deref().filter(|i| !i.is_empty()) {
        info["isrc"] = json!(cut(isrc));
    }
    let mut metadata = json!({
        "artist_name": cut(&artist),
        "track_name": cut(&title),
        "additional_info": info,
    });
    if !track.album.title.is_empty() {
        metadata["release_name"] = json!(cut(&track.album.title));
    }
    let mut listen = json!({ "track_metadata": metadata });
    if let Some(at) = listened_at {
        listen["listened_at"] = json!(at);
    }
    listen
}

/// Body of a `submit-listens` request.
pub fn submission(kind: ListenType, listens: Vec<Value>) -> Value {
    json!({ "listen_type": kind.as_str(), "payload": listens })
}

fn cut(s: &str) -> String {
    s.chars().take(MAX_FIELD_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::{AlbumRef, ArtistRef, TrackId};

    fn track() -> Track {
        Track {
            id: TrackId(42),
            title: "Time".into(),
            version: Some("Live".into()),
            duration: 276,
            artists: vec![
                ArtistRef {
                    id: 1,
                    name: "A".into(),
                },
                ArtistRef {
                    id: 2,
                    name: "B".into(),
                },
            ],
            album: AlbumRef {
                id: 3,
                title: "Inception".into(),
                cover_url: None,
                cover_small_url: None,
                cover_large_url: None,
                color: None,
            },
            explicit: false,
            ai: false,
            hires: true,
            isrc: Some("USAT21000001".into()),
        }
    }

    #[test]
    fn single_listen() {
        let l = listen(&track(), Some(1_700_000_000));
        assert_eq!(l["listened_at"], 1_700_000_000);
        let m = &l["track_metadata"];
        assert_eq!(m["track_name"], "Time (Live)");
        assert_eq!(m["artist_name"], "A, B");
        assert_eq!(m["release_name"], "Inception");
        let info = &m["additional_info"];
        assert_eq!(info["isrc"], "USAT21000001");
        assert_eq!(info["duration_ms"], 276_000);
        assert_eq!(info["music_service"], "tidal.com");
        assert_eq!(info["origin_url"], "https://tidal.com/track/42");
        assert_eq!(info["submission_client"], "Seaglass");
    }

    #[test]
    fn playing_now_has_no_timestamp() {
        let mut t = track();
        t.isrc = None;
        t.version = None;
        t.duration = 0;
        let l = listen(&t, None);
        assert!(l.get("listened_at").is_none());
        let m = &l["track_metadata"];
        assert_eq!(m["track_name"], "Time");
        assert!(m["additional_info"].get("isrc").is_none());
        assert!(m["additional_info"].get("duration_ms").is_none());
        let body = submission(ListenType::PlayingNow, vec![l]);
        assert_eq!(body["listen_type"], "playing_now");
        assert_eq!(body["payload"].as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn long_fields_stay_under_the_size_limit() {
        let mut t = track();
        t.title = "x".repeat(20_000);
        let l = listen(&t, Some(1));
        assert!(l.to_string().len() < 10_240);
    }
}
