//! Volume normalization (#82): ReplayGain gain from the API (not the FLAC tags), limited by
//! the peak so it never clips, applied by the engine **to each file** separately
//! (`Op::Play`, `Op::Preload`, `Op::CurrentFilters`), before the equalizer (#428,
//! `player::eq`).
//!
//! With mpv (`mpv::engine`), verified with mpv 0.41 and 0.37 (`live_per_file_af`):
//! `loadfile` with `options` `af=…` sets the filter as a local option of the preloaded
//! file; the current track keeps its own until the gapless switch, and a file loaded without
//! an option goes back to `af=[]`. For the current track, `file-local-options/af` changes
//! the filter live; the previous value is restored at the end of the file (`options/af`
//! shows the value in effect, not that one).

use serde::{Deserialize, Serialize};

use super::actor::Player;
use crate::tidal::models::ReplayGain;

/// Bounds of the applied gain, in dB: beyond them, an API value would be suspect.
const MIN_GAIN: f64 = -24.0;
const MAX_GAIN: f64 = 12.0;

/// Setting Settings › Playback › Volume normalization.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Normalization {
    /// Untouched signal (the only mode compatible with a bit-perfect output).
    #[default]
    Off,
    /// Every track at the same perceived volume.
    Track,
    /// Differences within an album kept, albums brought to the same volume.
    Album,
}

/// Gain to apply in dB, or `None` (setting off, unknown gain, or zero after rounding: the
/// signal is then left untouched). Album mode without an album gain: the track's gain. The
/// peak limits the gain: `peak × 10^(gain/20) ≤ 1`.
pub fn gain_db(mode: Normalization, rg: &ReplayGain) -> Option<f64> {
    let (gain, peak) = match mode {
        Normalization::Off => return None,
        Normalization::Album if rg.album_gain.is_some() => (rg.album_gain?, rg.album_peak),
        Normalization::Track | Normalization::Album => (rg.track_gain?, rg.track_peak),
    };
    if !gain.is_finite() {
        return None;
    }
    let ceiling = peak
        .filter(|p| p.is_finite() && *p > 0.0)
        .map_or(MAX_GAIN, |p| -20.0 * p.log10());
    let gain = gain.min(ceiling).clamp(MIN_GAIN, MAX_GAIN);
    let rounded = (gain * 100.0).round() / 100.0;
    (rounded != 0.0).then_some(rounded)
}

/// Normalization state held by the actor: ReplayGain of the current track and of the
/// preloaded next one, to recompute the gain when the setting changes.
#[derive(Debug, Clone, Default)]
pub(super) struct Normalizer {
    /// Chosen setting, kept as is during exclusive mode.
    pub(super) mode: Normalization,
    /// Exclusive mode (#245): no filter set, whatever the setting.
    pub(super) bypass: bool,
    current: ReplayGain,
    next: ReplayGain,
}

impl Normalizer {
    pub(super) fn new(mode: Normalization, bypass: bool) -> Self {
        Self {
            mode,
            bypass,
            ..Self::default()
        }
    }

    /// Normalization actually applied: none in exclusive mode.
    pub(super) fn effective(&self) -> Normalization {
        if self.bypass {
            Normalization::Off
        } else {
            self.mode
        }
    }

    /// Track loaded at entry 0: gain to apply to it.
    pub(super) fn loaded(&mut self, rg: ReplayGain) -> Option<f64> {
        self.current = rg;
        gain_db(self.effective(), &rg)
    }

    /// Track preloaded at entry 1: gain set with its `loadfile`.
    pub(super) fn preloaded(&mut self, rg: ReplayGain) -> Option<f64> {
        self.next = rg;
        gain_db(self.effective(), &rg)
    }

    /// Gapless switch to the preloaded track: its gain becomes the current one.
    pub(super) fn advance(&mut self) -> Option<f64> {
        self.current = std::mem::take(&mut self.next);
        gain_db(self.effective(), &self.current)
    }

    pub(super) fn current_gain(&self) -> Option<f64> {
        gain_db(self.effective(), &self.current)
    }
}

impl Player {
    /// New setting: applied right away to the current track, and the preloaded next one is
    /// removed so it is reloaded with its gain.
    pub(super) async fn set_normalization(&mut self, mode: Normalization) {
        let before = self.normalizer.effective();
        self.normalizer.mode = mode;
        self.note_device_profile();
        self.on_normalization_changed(before).await;
    }

    /// Reapplies the gain if the effective normalization changed (setting, or switching to
    /// exclusive mode and back).
    pub(super) async fn on_normalization_changed(&mut self, before: Normalization) {
        if self.normalizer.effective() == before {
            return;
        }
        self.refresh_filters().await;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn rg(track: (f64, f64), album: Option<(f64, f64)>) -> ReplayGain {
        ReplayGain {
            track_gain: Some(track.0),
            track_peak: Some(track.1),
            album_gain: album.map(|a| a.0),
            album_peak: album.map(|a| a.1),
        }
    }

    #[test]
    fn gain_follows_mode_and_falls_back_to_track() {
        let r = rg((-6.5, 0.9), Some((-4.0, 0.99)));
        assert_eq!(gain_db(Normalization::Off, &r), None);
        assert_eq!(gain_db(Normalization::Track, &r), Some(-6.5));
        assert_eq!(gain_db(Normalization::Album, &r), Some(-4.0));
        let no_album = rg((-6.5, 0.9), None);
        assert_eq!(gain_db(Normalization::Album, &no_album), Some(-6.5));
        assert_eq!(gain_db(Normalization::Track, &ReplayGain::default()), None);
    }

    #[test]
    fn peak_prevents_clipping() {
        // +6 dB requested, peak at 0.7: at most −20·log10(0.7) ≈ +3.10 dB.
        let g = gain_db(Normalization::Track, &rg((6.0, 0.7), None)).unwrap();
        assert!((g - 3.1).abs() < 0.01, "{g}");
        assert!(0.7 * 10f64.powf(g / 20.0) <= 1.0 + 1e-3);
        // Full-scale peak: no positive gain, an attenuation goes through as is.
        assert_eq!(gain_db(Normalization::Track, &rg((2.0, 1.0), None)), None);
        assert_eq!(
            gain_db(Normalization::Track, &rg((-3.0, 1.0), None)),
            Some(-3.0)
        );
        // Unknown or absurd peak: only the bounds.
        let no_peak = ReplayGain {
            track_gain: Some(20.0),
            ..ReplayGain::default()
        };
        assert_eq!(gain_db(Normalization::Track, &no_peak), Some(MAX_GAIN));
        assert_eq!(
            gain_db(Normalization::Track, &rg((-40.0, 0.0), None)),
            Some(MIN_GAIN)
        );
        assert_eq!(
            gain_db(Normalization::Track, &rg((f64::NAN, 0.5), None)),
            None
        );
    }

    #[test]
    fn normalizer_tracks_current_and_next() {
        let mut n = Normalizer::new(Normalization::Track, false);
        assert_eq!(n.loaded(rg((-5.0, 0.5), None)), Some(-5.0));
        assert_eq!(n.preloaded(rg((-2.0, 0.5), None)), Some(-2.0));
        assert_eq!(n.current_gain(), Some(-5.0));
        assert_eq!(n.advance(), Some(-2.0));
        assert_eq!(n.current_gain(), Some(-2.0));
        // No known preloaded track: nothing to apply after the switch.
        assert_eq!(n.advance(), None);
        n.mode = Normalization::Off;
        assert_eq!(n.loaded(rg((-5.0, 0.5), None)), None);
    }

    /// Exclusive mode (#245): no gain, the setting is kept for the return to shared mode.
    #[test]
    fn exclusive_bypasses_normalization() {
        let mut n = Normalizer::new(Normalization::Album, true);
        assert_eq!(n.effective(), Normalization::Off);
        assert_eq!(n.loaded(rg((-5.0, 0.5), None)), None);
        assert_eq!(n.preloaded(rg((-2.0, 0.5), None)), None);
        n.bypass = false;
        assert_eq!(n.effective(), Normalization::Album);
        assert_eq!(n.current_gain(), Some(-5.0));
    }

    #[test]
    fn setting_serialization() {
        assert_eq!(
            serde_json::to_string(&Normalization::Album).unwrap(),
            r#""album""#
        );
        assert!(serde_json::from_str::<Normalization>(r#""loud""#).is_err());
    }

    /// 16-bit stereo 44.1 kHz PCM WAV: sine at −40 dBFS (low volume), test file.
    pub(crate) fn quiet_wav(name: &str, secs: u32, freq: f64) -> std::path::PathBuf {
        let rate = 44_100u32;
        let frames = rate * secs;
        let data_len = frames * 4;
        let mut wav = Vec::with_capacity(44 + data_len as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        for (v, n) in [
            (16u32, 4),
            (1, 2),
            (2, 2),
            (rate, 4),
            (rate * 4, 4),
            (4, 2),
            (16, 2),
        ] {
            wav.extend_from_slice(&v.to_le_bytes()[..n]);
        }
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        for i in 0..frames {
            let s = (0.01
                * 32767.0
                * (std::f64::consts::TAU * freq * f64::from(i) / f64::from(rate)).sin())
                as i16;
            wav.extend_from_slice(&s.to_le_bytes());
            wav.extend_from_slice(&s.to_le_bytes());
        }
        let path = std::env::temp_dir().join(format!("seaglass-{name}-{}.wav", std::process::id()));
        std::fs::write(&path, &wav).unwrap();
        path
    }

    /// Real mpv, short real playback at low volume (sine at −40 dBFS, `volume=30`): the
    /// filter set by `loadfile` (`append`, `options` `af=…`, commands of `mpv::engine`) only
    /// applies to the preloaded file, the current track keeps its own until the gapless
    /// switch; `file-local-options/af` changes the current track's filter without spilling
    /// over to the next ones (the preloaded one keeps its own, a file loaded afterwards
    /// without an option goes back to `af=[]`). With the equalizer (#428): the multi-filter
    /// chain goes through `options` as `%length%value`.
    /// `cargo test live_per_file_af -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "launches a real mpv on the audio output"]
    async fn live_per_file_af() {
        use crate::engine::{Filters, Op};
        use crate::mpv::CacheSize;
        use crate::mpv::engine::commands;
        use crate::mpv::events::{MpvEvent, Property};
        use crate::mpv::filters::af_value;
        use crate::mpv::filters::tests::hd600;
        use serde_json::{Value, json};
        use std::time::Duration;

        let conf = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("mpv/seaglass.conf");
        let mut child = crate::mpv::process::spawn(&conf, CacheSize::Mib32, None, false).unwrap();
        let (events_tx, mut events) = tokio::sync::mpsc::channel(256);
        let handle = crate::mpv::ipc::connect(events_tx).await.unwrap();
        let cmd = |c: Value| {
            let handle = handle.clone();
            async move { handle.command(c).await.unwrap() }
        };
        let af = || cmd(json!(["get_property", "af"]));
        let volume_of = |v: Value| -> Option<String> {
            v.as_array()?
                .iter()
                .find(|f| f["name"] == "volume")
                .and_then(|f| f["params"]["volume"].as_str().map(str::to_owned))
        };
        let names = |v: Value| -> Vec<String> {
            v.as_array()
                .map(|l| {
                    l.iter()
                        .filter_map(|f| f["name"].as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default()
        };
        let with_eq = |gain| Filters {
            gain: Some(gain),
            eq: Some(hd600()),
        };
        cmd(json!(["set_property", "volume", 30])).await;

        let a = quiet_wav("af-a", 3, 440.0);
        let b = quiet_wav("af-b", 3, 660.0);
        let c = quiet_wav("af-c", 2, 550.0);
        let path = |p: &std::path::PathBuf| json!(p.to_string_lossy());
        let url = |p: &std::path::PathBuf| p.to_string_lossy().into_owned();

        let play = Op::Play {
            url: url(&a),
            start: 0.0,
            filters: with_eq(-6.0),
        };
        let preload = Op::Preload {
            url: url(&b),
            filters: Filters {
                gain: Some(-12.0),
                eq: None,
            },
        };
        for c in commands(&play).into_iter().chain(commands(&preload)) {
            cmd(c).await;
        }
        tokio::time::sleep(Duration::from_millis(800)).await;
        let on_a = af().await;
        println!("A: af = {on_a}");
        assert_eq!(
            volume_of(on_a.clone()).as_deref(),
            Some("-12.20dB"),
            "current track: −6 dB gain + −6.2 dB preamp"
        );
        assert_eq!(
            names(on_a),
            ["volume", "bass", "equalizer", "treble"],
            "whole equalizer chain despite the commas"
        );
        let params = cmd(json!(["get_property", "audio-params"])).await;
        println!("A: preloaded B waiting, audio-params = {params}");

        // Live change of the current track: local option only.
        cmd(json!([
            "set_property",
            "file-local-options/af",
            af_value(&with_eq(-3.0))
        ]))
        .await;
        let hot = af().await;
        assert_eq!(volume_of(hot.clone()).as_deref(), Some("-9.20dB"));
        assert_eq!(names(hot).len(), 4);

        // Gapless switch to B: its own filter.
        tokio::time::timeout(Duration::from_secs(6), async {
            while let Some(ev) = events.recv().await {
                if ev == MpvEvent::Property(Property::PlaylistPos(Some(1))) {
                    return;
                }
            }
        })
        .await
        .expect("switch to the preloaded track");
        tokio::time::sleep(Duration::from_millis(300)).await;
        let on_b = volume_of(af().await);
        println!("B after gapless switch: {on_b:?}");
        assert_eq!(on_b.as_deref(), Some("-12.00dB"));
        assert_eq!(names(af().await), ["volume"], "B without equalizer");

        // File without an option: no filter.
        cmd(json!(["loadfile", path(&c), "replace"])).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(
            af().await,
            json!([]),
            "without an option, no more filter: the live change did not spill over"
        );

        let _ = child.start_kill();
        for p in [a, b, c] {
            let _ = std::fs::remove_file(p);
        }
    }
}
