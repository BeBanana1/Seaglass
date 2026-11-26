//! Track details: lyrics, track info, credits, contributors, hi-res edition.

use serde::Serialize;

use super::{ArtistSummary, StreamFormat, Track, TrackId};

/// Lyrics of a track (`tracks/{id}/lyrics`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lyrics {
    pub track_id: TrackId,
    /// Provider to credit ("MUSIXMATCH").
    pub provider: Option<String>,
    /// Right-to-left script.
    pub rtl: bool,
    pub text: LyricsText,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LyricsText {
    /// Timed lines, sorted by time; an empty line marks an instrumental passage.
    Synced { lines: Vec<LyricLine> },
    /// Text without timing, line by line (empty lines = break between verses).
    Plain { lines: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    /// Start of the line, in milliseconds.
    pub time_ms: u32,
    pub text: String,
}

/// Detailed info of a track (#3), asked for when the panel opens.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackInfo {
    pub track: Track,
    /// Availability tags (`LOSSLESS`, `HIRES_LOSSLESS`, `DOLBY_ATMOS`…).
    pub tags: Vec<String>,
    /// Format served at maximum quality, as for playback; `None` if refused.
    pub stream: Option<StreamFormat>,
    /// Why the stream was refused (DRM, track unavailable…).
    pub stream_error: Option<String>,
    /// ReplayGain (dB) and peaks (full scale = 1), read but never applied.
    pub track_gain: Option<f64>,
    pub track_peak: Option<f64>,
    pub album_gain: Option<f64>,
    pub album_peak: Option<f64>,
    pub bpm: Option<u32>,
    /// Key as given by the API (`B`, `FSharp`…) and its mode (`MAJOR`, `MINOR`).
    pub key: Option<String>,
    pub key_scale: Option<String>,
    pub copyright: Option<String>,
    pub track_number: Option<u32>,
    pub volume_number: Option<u32>,
    /// Other hi-res edition of the same recording (#18, #55), if the track is not hi-res.
    pub hires_edition: Option<HiresEdition>,
    /// Credits grouped by role (#84); empty if TIDAL has none.
    pub credits: Vec<CreditGroup>,
}

/// A role (`Composer`, `Producer`, `Bass guitar`…, free English text from the API) and its
/// contributors.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditGroup {
    pub role: String,
    pub contributors: Vec<Contributor>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Contributor {
    pub name: String,
    /// Matching TIDAL artist (artist page), if there is one.
    pub artist_id: Option<u64>,
}

/// Credits of a whole album (#206): a role and its contributors, with the tracks each one
/// appears on.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumCreditGroup {
    pub role: String,
    pub contributors: Vec<AlbumContributor>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumContributor {
    pub name: String,
    pub artist_id: Option<u64>,
    /// Tracks they appear on, in album order; empty = every track. Identifiers rather than
    /// numbers: the UI places them in the list it shows.
    pub tracks: Vec<TrackId>,
}

/// Page of a contributor (#429): tracks a person is credited on, in batches.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContributorPage {
    pub artist: ArtistSummary,
    /// Number of tracks announced by the service, for the whole catalog.
    pub total: u32,
    pub tracks: Vec<Contribution>,
    /// To give back to `contributor` for the next batch; `None` at the end.
    pub next_offset: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Contribution {
    pub track: Track,
    /// Roles on this track (`Producer`, `Composer`…, free English text from the API).
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HiresEdition {
    pub track: Track,
    pub stream: Option<StreamFormat>,
    /// Seaglass plays it instead of the track (setting on and stream actually better).
    pub played: bool,
}
