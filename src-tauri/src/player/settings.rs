//! Playback settings, stored apart from the UI settings (`playback.settings` key of the
//! `kv` table): the player reads them before launching mpv.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use super::eq::EqSettings;
use super::normalize::Normalization;
use crate::mpv::{AudioDevice, CacheSize};
use crate::store::Store;
use crate::tidal::models::AudioQuality;

const KEY: &str = "playback.settings";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlaybackSettings {
    /// Size of mpv's cache, in MiB (#16).
    pub cache_size: CacheSize,
    /// Track without a hi-res version: play the hi-res edition of the same recording if it
    /// exists (#18), or offer it (#59).
    pub hires_upgrade: HiresUpgrade,
    /// Volume normalization through ReplayGain (#82), off by default.
    pub normalization: Normalization,
    /// Parametric equalizer (#428, Android #467), off by default.
    pub equalizer: EqSettings,
    /// Chosen output device (#79); `None` = the system's default device. Kept even when
    /// unplugged: the player falls back to the default and returns to it when it is plugged
    /// back in.
    pub audio_device: Option<AudioDevice>,
    /// WASAPI exclusive mode (#245), off by default: other apps get no sound during
    /// playback, and normalization is suspended. No effect on Android.
    pub exclusive: bool,
    /// End of queue: carry on with the last track's radio (#78). Off by default.
    pub auto_radio: bool,
    /// Desktop: pause when the output device in use disappears (headset unplugged, #348). On
    /// by default.
    pub pause_on_unplug: bool,
    /// Desktop: resume playback when the app launches (#353). Off by default.
    pub resume_on_launch: bool,
    /// Desktop: volume, normalization, exclusive mode and equalizer profile kept per output
    /// device (#416, `player::profiles`). Off by default: global settings.
    pub device_profiles: bool,
    /// Android: quality requested from TIDAL on an unmetered network (Wi-Fi, #365, #366),
    /// Lossless by default. No effect on desktop, which is always Hi-Res Lossless.
    pub wifi_quality: MobileQuality,
    /// Android: quality on a metered network (mobile data, tethering, #365).
    pub cellular_quality: MobileQuality,
    /// Android: cap the quality when the battery is low (#365, `player::quality`). Off by
    /// default.
    pub low_battery_quality: bool,
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self {
            cache_size: CacheSize::default(),
            hires_upgrade: HiresUpgrade::default(),
            normalization: Normalization::default(),
            equalizer: EqSettings::default(),
            audio_device: None,
            exclusive: false,
            auto_radio: false,
            pause_on_unplug: true,
            resume_on_launch: false,
            device_profiles: false,
            wifi_quality: MobileQuality::default(),
            cellular_quality: MobileQuality::default(),
            low_battery_quality: false,
        }
    }
}

/// Playback quality on Android (#325, #366), the tiers of the TIDAL API. Listening there
/// often goes through Bluetooth, which re-encodes the sound: "High" (AAC 320 kbit/s) needs
/// about three times less data than FLAC, "Low" (HE-AAC 96 kbit/s) ten times less.
/// Ascending order: `Ord` is used to cap (#365).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MobileQuality {
    Low,
    High,
    #[default]
    Lossless,
    /// FLAC up to 24 bits / 192 kHz, hi-res edition included as on desktop. Android brings
    /// 24 bits down to 16 bits (no float output), the sample rate stays.
    Max,
}

impl MobileQuality {
    pub fn audio_quality(self) -> AudioQuality {
        match self {
            Self::Low => AudioQuality::Low,
            Self::High => AudioQuality::High,
            Self::Lossless => AudioQuality::Lossless,
            Self::Max => AudioQuality::HiResLossless,
        }
    }
}

/// What to do with a hi-res edition found for a track that has none.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HiresUpgrade {
    /// Play it instead, without asking (shown in the signal path).
    #[default]
    Auto,
    /// Play the original track and offer the edition in the player bar.
    Ask,
    Off,
}

/// Also accepts the on/off switch of versions 0.4.8 to 0.4.14 (`true` = automatic).
impl<'de> Deserialize<'de> for HiresUpgrade {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Legacy(bool),
            Mode(String),
        }
        match Repr::deserialize(deserializer)? {
            Repr::Legacy(true) => Ok(Self::Auto),
            Repr::Legacy(false) => Ok(Self::Off),
            Repr::Mode(mode) => match mode.as_str() {
                "auto" => Ok(Self::Auto),
                "ask" => Ok(Self::Ask),
                "off" => Ok(Self::Off),
                _ => Err(D::Error::unknown_variant(&mode, &["auto", "ask", "off"])),
            },
        }
    }
}

impl PlaybackSettings {
    /// Saved settings; defaults without a database or if the value is unreadable (old
    /// version, size removed from the choices).
    pub fn load(store: Option<&Store>) -> Self {
        let Some(store) = store else {
            return Self::default();
        };
        let saved = store
            .get_setting::<serde_json::Value>(KEY)
            .map_err(|e| e.to_string())
            .and_then(|saved| {
                saved
                    .map(Self::from_saved)
                    .transpose()
                    .map_err(|e| e.to_string())
            });
        match saved {
            Ok(saved) => saved.unwrap_or_default(),
            Err(e) => {
                tracing::warn!("unreadable playback settings, using defaults: {e}");
                Self::default()
            }
        }
    }

    /// Reads the saved settings. `mobileQuality` (a single quality, 0.5.22 to 0.5.29) becomes
    /// the quality of both networks (#365).
    fn from_saved(mut saved: serde_json::Value) -> serde_json::Result<Self> {
        if let Some(map) = saved.as_object_mut()
            && let Some(old) = map.remove("mobileQuality")
        {
            map.entry("wifiQuality").or_insert_with(|| old.clone());
            map.entry("cellularQuality").or_insert(old);
        }
        serde_json::from_value(saved)
    }

    pub fn save(&self, store: &Store) -> crate::store::Result<()> {
        store.set_setting(KEY, self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_default_and_invalid_size_is_rejected() {
        let s: PlaybackSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(s.cache_size, CacheSize::Mib64);
        assert_eq!(
            s.hires_upgrade,
            HiresUpgrade::Auto,
            "automatic by default, including since a 0.4.7"
        );
        assert_eq!(s.normalization, Normalization::Off, "off by default");
        let s: PlaybackSettings = serde_json::from_str(r#"{"cacheSize":256}"#).unwrap();
        assert_eq!(s.cache_size, CacheSize::Mib256);
        assert!(serde_json::from_str::<PlaybackSettings>(r#"{"cacheSize":100}"#).is_err());
        assert_eq!(
            serde_json::to_string(&PlaybackSettings::default()).unwrap(),
            r#"{"cacheSize":64,"hiresUpgrade":"auto","normalization":"off","equalizer":{"enabled":false,"active":null,"profiles":[]},"audioDevice":null,"exclusive":false,"autoRadio":false,"pauseOnUnplug":true,"resumeOnLaunch":false,"deviceProfiles":false,"wifiQuality":"lossless","cellularQuality":"lossless","lowBatteryQuality":false}"#
        );
        assert!(!s.auto_radio, "automatic radio off by default");
        assert!(!s.exclusive, "exclusive mode off by default");
        assert!(s.pause_on_unplug, "pause on unplug by default (#348)");
        assert!(!s.resume_on_launch, "no resume at launch by default (#353)");
        assert!(!s.device_profiles, "global settings by default (#416)");
    }

    #[test]
    fn mobile_quality_is_lossless_unless_asked() {
        let read =
            |json: &str| serde_json::from_str::<PlaybackSettings>(json).map(|s| s.wifi_quality);
        assert_eq!(read("{}").unwrap(), MobileQuality::Lossless);
        assert_eq!(
            read(r#"{"wifiQuality":"high"}"#).unwrap(),
            MobileQuality::High
        );
        assert_eq!(
            read(r#"{"wifiQuality":"low"}"#).unwrap(),
            MobileQuality::Low
        );
        assert_eq!(
            read(r#"{"wifiQuality":"max"}"#).unwrap(),
            MobileQuality::Max
        );
        assert!(read(r#"{"wifiQuality":"hires"}"#).is_err());
        assert_eq!(MobileQuality::Low.audio_quality(), AudioQuality::Low);
        assert_eq!(MobileQuality::High.audio_quality(), AudioQuality::High);
        assert_eq!(
            MobileQuality::Lossless.audio_quality(),
            AudioQuality::Lossless
        );
        assert_eq!(
            MobileQuality::Max.audio_quality(),
            AudioQuality::HiResLossless
        );
        assert!(MobileQuality::Low < MobileQuality::High);
        assert!(MobileQuality::Lossless < MobileQuality::Max);
    }

    #[test]
    fn single_mobile_quality_becomes_both_networks() {
        let read = |json: &str| {
            let s = PlaybackSettings::from_saved(serde_json::from_str(json).unwrap()).unwrap();
            (s.wifi_quality, s.cellular_quality, s.low_battery_quality)
        };
        assert_eq!(
            read(r#"{"mobileQuality":"high"}"#),
            (MobileQuality::High, MobileQuality::High, false)
        );
        assert_eq!(
            read(r#"{"wifiQuality":"max","cellularQuality":"low","lowBatteryQuality":true}"#),
            (MobileQuality::Max, MobileQuality::Low, true)
        );
        assert_eq!(
            read("{}"),
            (MobileQuality::Lossless, MobileQuality::Lossless, false)
        );
        assert!(PlaybackSettings::from_saved(serde_json::json!({"mobileQuality": "x"})).is_err());
    }

    #[test]
    fn audio_device_is_validated() {
        let s: PlaybackSettings = serde_json::from_str(
            r#"{"audioDevice":{"id":"wasapi/{27264a86-c734-4c85-876c-dcfeb567529f}","name":"DAC"}}"#,
        )
        .unwrap();
        assert_eq!(s.audio_device.map(|d| d.name), Some("DAC".into()));
        assert!(
            serde_json::from_str::<PlaybackSettings>(
                r#"{"audioDevice":{"id":"openal","name":"x"}}"#
            )
            .is_err()
        );
    }

    #[test]
    fn hires_upgrade_reads_the_old_switch() {
        let read =
            |json: &str| serde_json::from_str::<PlaybackSettings>(json).map(|s| s.hires_upgrade);
        assert_eq!(
            read(r#"{"hiresUpgrade":true}"#).unwrap(),
            HiresUpgrade::Auto
        );
        assert_eq!(
            read(r#"{"hiresUpgrade":false}"#).unwrap(),
            HiresUpgrade::Off
        );
        assert_eq!(
            read(r#"{"hiresUpgrade":"ask"}"#).unwrap(),
            HiresUpgrade::Ask
        );
        assert_eq!(
            read(r#"{"hiresUpgrade":"off"}"#).unwrap(),
            HiresUpgrade::Off
        );
        assert!(read(r#"{"hiresUpgrade":"always"}"#).is_err());
        assert!(read(r#"{"hiresUpgrade":1}"#).is_err());
    }
}
