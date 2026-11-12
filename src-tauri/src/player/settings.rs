//! Réglages de lecture, stockés à part des réglages d'UI (clé `playback.settings` de la
//! table `kv`) : le lecteur les lit avant de lancer mpv.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use super::normalize::Normalization;
use crate::mpv::{AudioDevice, CacheSize};
use crate::store::Store;
use crate::tidal::models::AudioQuality;

const KEY: &str = "playback.settings";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlaybackSettings {
    /// Taille du cache mpv, en Mio (#16).
    pub cache_size: CacheSize,
    /// Titre sans version hi-res : lire l'édition hi-res du même enregistrement si elle
    /// existe (#18), ou la proposer (#59).
    pub hires_upgrade: HiresUpgrade,
    /// Normalisation du volume par ReplayGain (#82), désactivée par défaut.
    pub normalization: Normalization,
    /// Périphérique de sortie choisi (#79) ; `None` = périphérique par défaut du système.
    /// Gardé même débranché : le lecteur se replie sur le défaut et y revient au
    /// rebranchement.
    pub audio_device: Option<AudioDevice>,
    /// Mode exclusif WASAPI (#245), désactivé par défaut : les autres apps n'ont plus de son
    /// pendant la lecture, et la normalisation est suspendue. Sans effet sur Android.
    pub exclusive: bool,
    /// Fin de file : enchaîner sur la radio du dernier titre (#78). Désactivé par défaut.
    pub auto_radio: bool,
    /// Bureau : mettre en pause quand le périphérique de sortie utilisé disparaît (casque
    /// débranché, #348). Activé par défaut.
    pub pause_on_unplug: bool,
    /// Bureau : reprendre la lecture au lancement de l'app (#353). Désactivé par défaut.
    pub resume_on_launch: bool,
    /// Android : qualité demandée à TIDAL sur un réseau non facturé à l'usage (Wi-Fi, #365,
    /// #366), Lossless par défaut. Sans effet sur le bureau, toujours en Hi-Res Lossless.
    pub wifi_quality: MobileQuality,
    /// Android : qualité sur un réseau facturé (données mobiles, partage de connexion, #365).
    pub cellular_quality: MobileQuality,
    /// Android : plafonner la qualité quand la batterie est faible (#365, `player::quality`).
    /// Désactivé par défaut.
    pub low_battery_quality: bool,
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self {
            cache_size: CacheSize::default(),
            hires_upgrade: HiresUpgrade::default(),
            normalization: Normalization::default(),
            audio_device: None,
            exclusive: false,
            auto_radio: false,
            pause_on_unplug: true,
            resume_on_launch: false,
            wifi_quality: MobileQuality::default(),
            cellular_quality: MobileQuality::default(),
            low_battery_quality: false,
        }
    }
}

/// Qualité de lecture sur Android (#325, #366), les paliers de l'API TIDAL. L'écoute y passe
/// souvent par le Bluetooth, qui réencode le son : « Haute » (AAC 320 kbit/s) demande
/// environ trois fois moins de données que le FLAC, « Basse » (HE-AAC 96 kbit/s) dix fois
/// moins. Ordre croissant : `Ord` sert à plafonner (#365).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MobileQuality {
    Low,
    High,
    #[default]
    Lossless,
    /// FLAC jusqu'à 24 bits / 192 kHz, édition hi-res comprise comme sur le bureau. Android
    /// ramène le 24 bits à 16 bits (pas de sortie flottante), la fréquence reste.
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

/// Que faire d'une édition hi-res trouvée pour un titre qui n'en a pas.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HiresUpgrade {
    /// La lire à sa place, sans demander (indiquée dans la chaîne audio).
    #[default]
    Auto,
    /// Lire le titre d'origine et proposer l'édition dans la barre de lecture.
    Ask,
    Off,
}

/// Accepte aussi l'interrupteur des versions 0.4.8 à 0.4.14 (`true` = automatique).
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
    /// Réglages enregistrés ; valeurs par défaut sans base ou si la valeur est illisible
    /// (ancienne version, taille retirée des choix).
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
                tracing::warn!("réglages de lecture illisibles, valeurs par défaut : {e}");
                Self::default()
            }
        }
    }

    /// Lit les réglages enregistrés. `mobileQuality` (une seule qualité, 0.5.22 à 0.5.29)
    /// devient la qualité des deux réseaux (#365).
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
            "automatique par défaut, y compris depuis une 0.4.7"
        );
        assert_eq!(s.normalization, Normalization::Off, "désactivée par défaut");
        let s: PlaybackSettings = serde_json::from_str(r#"{"cacheSize":256}"#).unwrap();
        assert_eq!(s.cache_size, CacheSize::Mib256);
        assert!(serde_json::from_str::<PlaybackSettings>(r#"{"cacheSize":100}"#).is_err());
        assert_eq!(
            serde_json::to_string(&PlaybackSettings::default()).unwrap(),
            r#"{"cacheSize":64,"hiresUpgrade":"auto","normalization":"off","audioDevice":null,"exclusive":false,"autoRadio":false,"pauseOnUnplug":true,"resumeOnLaunch":false,"wifiQuality":"lossless","cellularQuality":"lossless","lowBatteryQuality":false}"#
        );
        assert!(!s.auto_radio, "radio automatique désactivée par défaut");
        assert!(!s.exclusive, "mode exclusif désactivé par défaut");
        assert!(
            s.pause_on_unplug,
            "pause au débranchement par défaut (#348)"
        );
        assert!(
            !s.resume_on_launch,
            "pas de reprise au lancement par défaut (#353)"
        );
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
