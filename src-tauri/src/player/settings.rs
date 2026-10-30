//! Réglages de lecture, stockés à part des réglages d'UI (clé `playback.settings` de la
//! table `kv`) : le lecteur les lit avant de lancer mpv.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use crate::mpv::CacheSize;
use crate::store::Store;

const KEY: &str = "playback.settings";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlaybackSettings {
    /// Taille du cache mpv, en Mio (#16).
    pub cache_size: CacheSize,
    /// Titre sans version hi-res : lire l'édition hi-res du même enregistrement si elle
    /// existe (#18), ou la proposer (#59).
    pub hires_upgrade: HiresUpgrade,
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
        match store.get_setting::<Self>(KEY) {
            Ok(saved) => saved.unwrap_or_default(),
            Err(e) => {
                tracing::warn!("réglages de lecture illisibles, valeurs par défaut : {e}");
                Self::default()
            }
        }
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
        let s: PlaybackSettings = serde_json::from_str(r#"{"cacheSize":256}"#).unwrap();
        assert_eq!(s.cache_size, CacheSize::Mib256);
        assert!(serde_json::from_str::<PlaybackSettings>(r#"{"cacheSize":100}"#).is_err());
        assert_eq!(
            serde_json::to_string(&PlaybackSettings::default()).unwrap(),
            r#"{"cacheSize":64,"hiresUpgrade":"auto"}"#
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
