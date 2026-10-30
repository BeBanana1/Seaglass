//! Réglages de lecture, stockés à part des réglages d'UI (clé `playback.settings` de la
//! table `kv`) : le lecteur les lit avant de lancer mpv.

use serde::{Deserialize, Serialize};

use crate::mpv::CacheSize;
use crate::store::Store;

const KEY: &str = "playback.settings";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlaybackSettings {
    /// Taille du cache mpv, en Mio (#16).
    pub cache_size: CacheSize,
    /// Titre sans version hi-res : lire l'édition hi-res du même enregistrement (même
    /// ISRC) si elle existe (#18).
    pub hires_upgrade: bool,
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self {
            cache_size: CacheSize::default(),
            hires_upgrade: true,
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
        assert!(
            s.hires_upgrade,
            "activé par défaut, y compris depuis une 0.4.7"
        );
        let s: PlaybackSettings = serde_json::from_str(r#"{"cacheSize":256}"#).unwrap();
        assert_eq!(s.cache_size, CacheSize::Mib256);
        assert!(serde_json::from_str::<PlaybackSettings>(r#"{"cacheSize":100}"#).is_err());
        assert_eq!(
            serde_json::to_string(&PlaybackSettings::default()).unwrap(),
            r#"{"cacheSize":64,"hiresUpgrade":true}"#
        );
    }
}
