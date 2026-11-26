//! States sent to the UI: `player-state` (frequent, light) and `queue-state`
//! (only when the queue changes, since it holds track lists).

use serde::Serialize;

use super::output::OutputDevice;
use super::queue::Repeat;
use super::sleep::SleepState;
use super::source::PlaySource;
use super::volume::Volume;
use crate::engine::AudioParams;
use crate::mpv::MpvIssue;
use crate::store::PlayFormat;
use crate::tidal::models::{StreamFormat, Track};

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export, export_to = "../../src/lib/bindings/")
)]
pub struct PlayerState {
    pub track: Option<Track>,
    /// Format served by TIDAL.
    pub stream: Option<StreamFormat>,
    /// Format actually sent to the audio output by mpv.
    pub output: Option<AudioParams>,
    /// Format actually written to the output (#245): Windows mix in shared mode, format
    /// opened on the device in exclusive mode. `None` on Android.
    pub device_format: Option<AudioParams>,
    pub output_driver: Option<String>,
    /// WASAPI exclusive mode active (#245).
    pub exclusive: bool,
    /// Output device actually in use (#79).
    pub output_device: Option<OutputDevice>,
    /// A track is being resolved (metadata + manifest).
    pub loading: bool,
    pub paused: bool,
    pub buffering: bool,
    pub idle: bool,
    /// Position in seconds; the UI interpolates between two events.
    pub time_pos: f64,
    pub duration: f64,
    /// Position up to which the current track is already in memory, in seconds (#453); 0 if
    /// unknown. Rounded to the second by `set_cached`.
    pub cached: f64,
    /// The next track is preloaded in the engine (2-entry window).
    pub next_ready: bool,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub has_previous: bool,
    pub has_next: bool,
    /// mpv software volume; any setting other than 100 unmuted is no longer bit-perfect.
    pub volume: Volume,
    /// Hi-res edition offered for the current track ("Ask" setting, #59): its format, the
    /// album in `hires_edition`.
    pub hires_offer: Option<StreamFormat>,
    /// Normalization gain applied to the current track, in dB (#82); `None` if the signal
    /// is not modified (setting off, gain unknown or zero).
    pub normalization: Option<f64>,
    /// Equalizer profile applied (#428); `None` if the equalizer is off.
    pub equalizer: Option<String>,
    /// Active sleep timer (#85).
    pub sleep: Option<SleepState>,
    /// mpv is not running (failed launch or stop), with its probable cause (#92). In the
    /// state rather than a toast: the launch failure comes before the webview listens.
    pub mpv_issue: Option<MpvIssue>,
    /// The current track comes from a chosen soundtrack (easter egg #399), recomputed on
    /// each emission.
    pub soundtrack: bool,
    /// The queue stopped by itself, nothing follows (easter egg #399); reset to false at
    /// the next load. `idle` is not enough: it is also true at startup.
    pub ended: bool,
    /// Blind test game in progress (#434): no `track` until the round is
    /// revealed.
    pub blind_test: bool,
}

impl PlayerState {
    /// True if only `time_pos` and `cached` differ: those changes are limited to
    /// ~4 Hz. Without copying the state (called on every position received, #147): both
    /// fields are swapped for the time of the comparison.
    pub fn differs_only_by_time(&mut self, other: &Self) -> bool {
        if self.time_pos == other.time_pos && self.cached == other.cached {
            return false;
        }
        let own_time = std::mem::replace(&mut self.time_pos, other.time_pos);
        let own_cached = std::mem::replace(&mut self.cached, other.cached);
        let same = *self == *other;
        self.time_pos = own_time;
        self.cached = own_cached;
        same
    }

    /// Applies the position held in memory (#453). The cache fills in small packets: only
    /// a gap of one second or more, a decrease (new track, seek) and the end of the track are
    /// kept, so as not to wake the UI on every packet.
    pub fn set_cached(&mut self, end: Option<f64>) {
        let end = end.unwrap_or(0.0).max(0.0);
        let reaches_end = self.duration > 0.0 && end >= self.duration - 1.0;
        if end < self.cached || end - self.cached >= 1.0 || (reaches_end && end != self.cached) {
            self.cached = end;
        }
    }

    /// Applies `audio-params`. mpv sets it to `null` at every end of file, but does not send
    /// the value again if the next file has the same format (the signal path is not
    /// reconfigured): a `null` is therefore ignored, the output is only cleared by `set_idle`.
    /// A format change is always notified.
    pub fn set_output(&mut self, params: Option<AudioParams>) {
        if params.is_some() {
            self.output = params;
        }
    }

    /// Applies `audio-out-params`, with the same rule as `set_output`: a `null` (end of file,
    /// output reopened) clears nothing.
    pub fn set_device_format(&mut self, params: Option<AudioParams>) {
        if params.is_some() {
            self.device_format = params;
        }
    }

    /// Format of the current listen, recorded with it (#524); `None` before the stream is
    /// known.
    pub fn played_format(&self) -> Option<PlayFormat> {
        let stream = self.stream.as_ref()?;
        Some(PlayFormat {
            quality: stream.quality.as_api_str().into(),
            codec: stream.codec.clone(),
            bit_depth: stream.bit_depth,
            sample_rate: stream.sample_rate,
            bit_perfect: self.bit_perfect(stream),
        })
    }

    /// Same rule as the UI's "Bit-perfect" verdict (`signalVerdict` in `lib/format.ts`):
    /// FLAC in exclusive mode, written at the stream's sample rate, not reduced to 16 bits,
    /// with no software volume, normalization or equalizer. The written format is the
    /// device's (#245), otherwise the decoder's.
    fn bit_perfect(&self, stream: &StreamFormat) -> bool {
        let Some(output) = self.device_format.as_ref().or(self.output.as_ref()) else {
            return false;
        };
        stream.codec == "flac"
            && self.exclusive
            && stream.sample_rate.is_none_or(|r| r == output.samplerate)
            && !(output.format == "s16" && stream.bit_depth.unwrap_or(16) > 16)
            && self.volume.is_unity()
            && self.normalization.is_none()
            && self.equalizer.is_none()
    }

    /// Applies `idle-active`: without a file, mpv has no audio output open anymore.
    pub fn set_idle(&mut self, idle: bool) {
        self.idle = idle;
        if idle {
            self.output = None;
            self.device_format = None;
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export, export_to = "../../src/lib/bindings/")
)]
pub struct QueueView {
    /// Manual queue ("Play next" / "Add to queue").
    pub manual: Vec<Track>,
    /// Next tracks of the context.
    pub upcoming: Vec<Track>,
    /// Starting track when the context is a radio that extends the queue (#78).
    pub radio: Option<Track>,
    /// Where the current track comes from ("Playing from", #124), if it comes from the context.
    pub source: Option<PlaySource>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_only_difference() {
        let mut a = PlayerState::default();
        let b = PlayerState {
            time_pos: 1.0,
            ..a.clone()
        };
        let c = PlayerState {
            time_pos: 1.0,
            paused: true,
            ..a.clone()
        };
        assert!(a.differs_only_by_time(&b));
        assert!(!a.differs_only_by_time(&c));
        let same = a.clone();
        assert!(!a.differs_only_by_time(&same));
        // The compared state is given back intact.
        assert_eq!(a, PlayerState::default());

        // The advancing cache is limited like the position.
        let d = PlayerState {
            time_pos: 1.0,
            cached: 30.0,
            ..a.clone()
        };
        assert!(a.differs_only_by_time(&d));
        assert_eq!(a, PlayerState::default());
    }

    #[test]
    fn cached_moves_by_whole_seconds() {
        let mut s = PlayerState {
            duration: 200.0,
            ..PlayerState::default()
        };
        s.set_cached(Some(10.0));
        assert_eq!(s.cached, 10.0);
        // Small packet: ignored.
        s.set_cached(Some(10.6));
        assert_eq!(s.cached, 10.0);
        s.set_cached(Some(11.2));
        assert_eq!(s.cached, 11.2);
        // End of the track reached: kept even under one second.
        s.set_cached(Some(198.8));
        s.set_cached(Some(199.5));
        assert_eq!(s.cached, 199.5);
        // New track or backward seek: always applied.
        s.set_cached(Some(3.0));
        assert_eq!(s.cached, 3.0);
        s.set_cached(None);
        assert_eq!(s.cached, 0.0);
    }

    fn params(samplerate: u32) -> Option<AudioParams> {
        Some(AudioParams {
            format: "s32".into(),
            samplerate,
            channel_count: 2,
        })
    }

    #[test]
    fn bit_perfect_follows_the_signal_path() {
        use crate::tidal::models::AudioQuality;
        let stream = StreamFormat {
            quality: AudioQuality::HiResLossless,
            codec: "flac".into(),
            bit_depth: Some(24),
            sample_rate: Some(96000),
            bitrate: None,
            hires_edition: None,
        };
        let clean = PlayerState {
            stream: Some(stream.clone()),
            device_format: params(96000),
            exclusive: true,
            ..PlayerState::default()
        };
        let format = clean.played_format().unwrap();
        assert_eq!(format.quality, "HI_RES_LOSSLESS");
        assert_eq!(
            (format.bit_depth, format.sample_rate),
            (Some(24), Some(96000))
        );
        assert!(format.bit_perfect);

        let not = |state: PlayerState| !state.played_format().unwrap().bit_perfect;
        assert!(not(PlayerState {
            exclusive: false,
            ..clean.clone()
        }));
        assert!(not(PlayerState {
            device_format: params(48000),
            ..clean.clone()
        }));
        assert!(not(PlayerState {
            device_format: Some(AudioParams {
                format: "s16".into(),
                samplerate: 96000,
                channel_count: 2,
            }),
            ..clean.clone()
        }));
        assert!(not(PlayerState {
            normalization: Some(-3.0),
            ..clean.clone()
        }));
        assert!(not(PlayerState {
            equalizer: Some("Harman".into()),
            ..clean.clone()
        }));
        let mut quiet = clean.clone();
        quiet.volume.set_level(80);
        assert!(not(quiet));
        assert!(not(PlayerState {
            device_format: None,
            output: None,
            ..clean.clone()
        }));
        // Decoder format when the device's is unknown.
        assert!(
            PlayerState {
                device_format: None,
                output: params(96000),
                ..clean.clone()
            }
            .played_format()
            .unwrap()
            .bit_perfect
        );
        assert_eq!(PlayerState::default().played_format(), None);
    }

    /// Track change with the same format: mpv sends `null` then nothing more (#12).
    #[test]
    fn output_survives_track_change_with_same_format() {
        let mut s = PlayerState::default();
        s.set_idle(false);
        s.set_output(params(96000));
        s.set_output(None);
        assert_eq!(s.output, params(96000));

        // A new format replaces the output.
        s.set_output(params(44100));
        assert_eq!(s.output, params(44100));

        // Nothing left to play: the output is closed.
        s.set_idle(true);
        assert_eq!(s.output, None);
        assert!(s.idle);
    }
}
