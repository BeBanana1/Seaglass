//! Streams: quality, format served, streaming session, play log, ReplayGain.

use serde::{Deserialize, Serialize};

use super::{Track, TrackId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export, export_to = "../../src/lib/bindings/")
)]
pub enum AudioQuality {
    Low,
    High,
    Lossless,
    HiResLossless,
}

impl AudioQuality {
    pub fn as_api_str(self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::High => "HIGH",
            Self::Lossless => "LOSSLESS",
            Self::HiResLossless => "HI_RES_LOSSLESS",
        }
    }
}

/// Format actually served by TIDAL for a given playback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export, export_to = "../../src/lib/bindings/")
)]
pub struct StreamFormat {
    pub quality: AudioQuality,
    pub codec: String,
    pub bit_depth: Option<u8>,
    pub sample_rate: Option<u32>,
    /// Average bitrate declared by the manifest, in kbps (#3).
    pub bitrate: Option<u32>,
    /// Album of the edition actually played when the stream comes from another hi-res edition
    /// of the same recording (#18); `None` for the track shown.
    pub hires_edition: Option<EditionAlbum>,
}

/// Album of another hi-res edition: shown and openable from "Now playing" (#111), without
/// changing the current track (like, playlists and queue keep its id).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export, export_to = "../../src/lib/bindings/")
)]
pub struct EditionAlbum {
    #[cfg_attr(test, ts(type = "number"))]
    pub id: u64,
    pub title: String,
}

impl From<&Track> for EditionAlbum {
    fn from(track: &Track) -> Self {
        Self {
            id: track.album.id,
            title: track.album.title.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum StreamSource {
    /// MPEG-DASH manifest (XML) served to mpv over local HTTP.
    Dash(String),
    /// Direct URL (BTS manifest).
    Direct(String),
}

#[derive(Debug, Clone)]
pub struct Stream {
    pub format: StreamFormat,
    pub source: StreamSource,
    /// ReplayGain of the stream actually played (hi-res edition included), for volume
    /// normalization (#82).
    pub gain: ReplayGain,
    /// Streaming session the play log reports this stream under (#34).
    pub session: PlaybackSession,
}

/// What a stream is requested for (#34): played now, or preloaded for gapless (sent as
/// `x-tidal-prefetch`, as the official clients do).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamUse {
    Play,
    Preload,
}

/// Streaming session of one stream request (#34): its id goes out with
/// `playbackinfopostpaywall` and comes back in the play log, with what TIDAL served.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackSession {
    /// UUID created by the client for this request.
    pub id: String,
    /// Track requested from TIDAL (a hi-res edition when the stream comes from one).
    pub requested: TrackId,
    /// Track TIDAL actually served (`trackId` of the answer).
    pub served: TrackId,
    /// `assetPresentation` of the answer (`FULL`, `PREVIEW`).
    pub presentation: String,
    /// `audioMode` of the answer (`STEREO`…).
    pub audio_mode: Option<String>,
    pub quality: AudioQuality,
}

/// Another client of the account started playing (#35, Pushkin's
/// `PRIVILEGED_SESSION_NOTIFICATION`): TIDAL allows one stream at a time, this one pauses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackTaken {
    /// What the other client calls itself (`clientDisplayName`: "Android" for the official
    /// app), when it says.
    pub device: Option<String>,
}

/// Where a listen was played from, as the play log names it (#34). Other sources are
/// reported under the track's album.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogSource {
    Album(u64),
    Playlist(String),
    Mix(String),
    Artist(u64),
}

/// Pause, resume or seek during a listen (#34), position in seconds, time in Unix ms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogAction {
    /// `PLAYBACK_START` if true, `PLAYBACK_STOP` otherwise.
    pub start: bool,
    pub position: f64,
    pub timestamp: i64,
}

/// One listen of one stream, from the first sound heard to the last (#34): the official
/// clients' `playback_session` event.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayLog {
    pub session: PlaybackSession,
    pub source: LogSource,
    pub start_timestamp: i64,
    pub start_position: f64,
    pub end_timestamp: i64,
    pub end_position: f64,
    pub actions: Vec<LogAction>,
}

/// ReplayGain from `playbackinfopostpaywall`: gains in dB, peaks in full scale (1 = 0 dBFS).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ReplayGain {
    pub track_gain: Option<f64>,
    pub track_peak: Option<f64>,
    pub album_gain: Option<f64>,
    pub album_peak: Option<f64>,
}
