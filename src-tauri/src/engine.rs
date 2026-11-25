//! Contract between the player actor and the engine that decodes the sound: mpv on desktop,
//! Media3 on Android (#24, `media3.rs`).
//!
//! The engine holds a **window of at most 2 entries**: the current track (0) and the
//! preloaded next one (1). The actor decides everything (queue, preload, errors); the
//! engine runs semantic operations (`Op`) and reports events (`Event`), knowing nothing
//! of TIDAL or the queue.
//!
//! The output device (`AudioDevice`) and the cache size (`CacheSize`) stay defined in
//! `mpv`: they are desktop settings, to revisit with the Android engine.

// On Android, Media3 has no device to choose nor list to report, and no exclusive
// mode: `End::OutputFailed`, `Event::Devices` and `Event::DeviceFormat` are not built
// there.
#![cfg_attr(mobile, allow(dead_code))]

use serde::{Deserialize, Serialize};

use std::sync::Arc;

use crate::mpv::{AudioDevice, CacheSize};
use crate::player::eq::EqProfile;

/// A file's filters: normalization gain (#82) then equalizer (#428), in that
/// order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Filters {
    pub gain: Option<f64>,
    pub eq: Option<Arc<EqProfile>>,
}

/// Operation asked of the engine.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// Replaces the whole window with `url` (entry 0) from `start` seconds, with its
    /// filters, and starts playback.
    Play {
        url: String,
        start: f64,
        filters: Filters,
    },
    /// Preloads `url` as entry 1 for a gapless transition, with its own filters (the
    /// current track keeps its own).
    Preload {
        url: String,
        filters: Filters,
    },
    /// Removes the preloaded track (the queue changed).
    RemoveNext,
    /// Forgets the finished track after `Event::Advanced`: the window is one entry again.
    DropPrevious,
    /// Stops playback and empties the window.
    Stop,
    TogglePause,
    SetPaused(bool),
    /// Absolute seek, in seconds.
    Seek(f64),
    /// Loops the current track (repeat One), without a reload.
    LoopCurrent(bool),
    /// Software volume 0–100 (100 = unity gain) and mute.
    Volume {
        level: u8,
        muted: bool,
    },
    CacheSize(CacheSize),
    /// New filters for the current track only (normalization or equalizer changed).
    CurrentFilters(Filters),
    /// Output device identifier (`auto` = system default).
    OutputDevice(String),
    /// WASAPI exclusive mode (#245): the engine reopens the output without interrupting the track.
    Exclusive(bool),
}

/// Decoded format sent to the output (mpv's `audio-params`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export, export_to = "../../src/lib/bindings/")
)]
pub struct AudioParams {
    pub format: String,
    pub samplerate: u32,
    #[serde(rename(deserialize = "channel-count"))]
    pub channel_count: u32,
}

/// End of a file that matters to the actor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    /// Natural end of the track.
    Eof,
    /// The output device could not open: fallback, not a stream error (#79).
    OutputFailed,
    /// Unreadable stream (expired URL, network…), with the engine's detail.
    Error(Option<String>),
}

/// Engine event, already translated.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Position(Option<f64>),
    /// Position (seconds) up to which the current track is already in memory (#453): mpv's
    /// demuxer cache, Media3's `bufferedPosition`. Nothing on disk.
    Cached(Option<f64>),
    Duration(Option<f64>),
    Paused(bool),
    Buffering(bool),
    /// `None` between two files: no effect on the display (#12).
    Format(Option<AudioParams>),
    /// Format actually written to the output (#245): Windows mix in shared mode, format
    /// opened on the device in exclusive mode. `None` while reopening.
    DeviceFormat(Option<AudioParams>),
    /// Output driver (`wasapi`…).
    Driver(Option<String>),
    Idle(bool),
    /// The engine moved to the preloaded entry, gaplessly.
    Advanced,
    /// File opened and decodable.
    Loaded,
    Ended(End),
    /// Known output devices, sent again on plugging (#79).
    Devices(Vec<AudioDevice>),
    /// The engine stopped (process ended or crashed).
    Stopped,
}

/// What an engine must be able to do. An error is an already translated message, shown
/// as a toast by the actor.
pub(crate) trait Engine {
    async fn apply(&self, op: Op) -> Result<(), String>;
}

#[cfg(mobile)]
pub use crate::media3::{Launch, Media3Engine as ActiveEngine};
#[cfg(desktop)]
pub use crate::mpv::engine::{Launch, MpvEngine as ActiveEngine};

/// Engine problem to show as a banner after a failed launch or a stop (#92).
pub fn diagnose() -> Option<crate::mpv::MpvIssue> {
    #[cfg(desktop)]
    return Some(crate::mpv::MpvIssue::diagnose());
    #[cfg(mobile)]
    None
}
