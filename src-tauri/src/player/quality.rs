//! Qualité demandée à TIDAL. Bureau : toujours Hi-Res Lossless. Android (#325, #365, #366) :
//! le réglage du réseau en cours (facturé à l'usage ou non), plafonné quand la batterie est
//! faible. Réseau et batterie sont relus à chaque résolution d'un titre : un changement
//! compte à partir du titre suivant, le titre en cours n'est jamais rechargé.

use serde::Deserialize;

use super::settings::{MobileQuality, PlaybackSettings};
use crate::tidal::models::AudioQuality;

// Lus seulement sur Android (et dans les tests) : le bureau ne choisit pas sa qualité.
/// Sous ce niveau de batterie (en %), au plus « Haute ».
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
const HIGH_CAP_BELOW: u8 = 50;
/// Sous ce niveau, ou économiseur de batterie d'Android actif, au plus « Basse ».
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
const LOW_CAP_BELOW: u8 = 20;

/// Règles tirées des réglages de lecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct QualityPolicy {
    wifi: MobileQuality,
    cellular: MobileQuality,
    low_battery: bool,
}

/// État de l'appareil lu par `DeviceConditions.kt`.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(super) struct Conditions {
    /// Réseau facturé à l'usage (données mobiles, partage de connexion) ou aucun réseau.
    pub metered: bool,
    /// Niveau en %, s'il est connu.
    pub battery: Option<u8>,
    /// Branché, même batterie pleine.
    pub charging: bool,
    /// Économiseur de batterie d'Android.
    pub power_save: bool,
}

impl QualityPolicy {
    pub fn new(settings: &PlaybackSettings) -> Self {
        Self {
            wifi: settings.wifi_quality,
            cellular: settings.cellular_quality,
            low_battery: settings.low_battery_quality,
        }
    }

    /// Meilleure qualité possible avec ces réglages : une édition hi-res n'est cherchée que
    /// si elle peut être lue.
    pub fn ceiling(self) -> AudioQuality {
        if cfg!(mobile) {
            self.wifi.max(self.cellular).audio_quality()
        } else {
            AudioQuality::HiResLossless
        }
    }

    /// Qualité pour cet état de l'appareil. Sans état (lecture impossible), la plus basse
    /// des deux qualités réseau : mieux vaut ne pas consommer de données par erreur. La
    /// batterie a le dernier mot sur le réseau.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn pick(self, conditions: Option<Conditions>) -> MobileQuality {
        let Some(c) = conditions else {
            return self.wifi.min(self.cellular);
        };
        let network = if c.metered { self.cellular } else { self.wifi };
        if !self.low_battery || c.charging {
            return network;
        }
        let below = |threshold: u8| c.battery.is_some_and(|level| level < threshold);
        let cap = if c.power_save || below(LOW_CAP_BELOW) {
            MobileQuality::Low
        } else if below(HIGH_CAP_BELOW) {
            MobileQuality::High
        } else {
            MobileQuality::Max
        };
        network.min(cap)
    }

    /// Qualité à demander pour le titre résolu maintenant.
    pub async fn current(self) -> AudioQuality {
        #[cfg(target_os = "android")]
        {
            let conditions = tauri_plugin_seaglass_mobile::direct::call::<Conditions>(
                "deviceConditions",
                serde_json::json!({}),
            )
            .await
            .inspect_err(|e| tracing::warn!("état du réseau et de la batterie illisible : {e}"))
            .ok();
            let quality = self.pick(conditions);
            tracing::debug!("qualité {quality:?} ({conditions:?})");
            quality.audio_quality()
        }
        #[cfg(not(target_os = "android"))]
        {
            self.ceiling()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use MobileQuality::{High, Lossless, Low, Max};

    fn policy(wifi: MobileQuality, cellular: MobileQuality, low_battery: bool) -> QualityPolicy {
        QualityPolicy {
            wifi,
            cellular,
            low_battery,
        }
    }

    fn on(metered: bool, battery: u8) -> Option<Conditions> {
        Some(Conditions {
            metered,
            battery: Some(battery),
            charging: false,
            power_save: false,
        })
    }

    #[test]
    fn network_picks_its_quality() {
        let p = policy(Max, High, false);
        assert_eq!(p.pick(on(false, 80)), Max, "Wi-Fi");
        assert_eq!(p.pick(on(true, 80)), High, "données mobiles");
        assert_eq!(
            p.pick(on(true, 5)),
            High,
            "batterie ignorée sans le réglage"
        );
        assert_eq!(p.pick(None), High, "état inconnu : la plus basse des deux");
    }

    #[test]
    fn low_battery_caps_the_network_quality() {
        let p = policy(Max, Lossless, true);
        assert_eq!(p.pick(on(false, 50)), Max);
        assert_eq!(p.pick(on(false, 49)), High);
        assert_eq!(p.pick(on(true, 49)), High);
        assert_eq!(p.pick(on(false, 20)), High);
        assert_eq!(p.pick(on(false, 19)), Low);
        // Le plafond ne relève jamais une qualité choisie plus basse.
        assert_eq!(policy(Low, Low, true).pick(on(false, 30)), Low);
        assert_eq!(policy(High, High, true).pick(on(false, 90)), High);
    }

    #[test]
    fn power_save_and_charging() {
        let p = policy(Lossless, Lossless, true);
        let saver = Conditions {
            power_save: true,
            ..on(false, 90).unwrap()
        };
        assert_eq!(p.pick(Some(saver)), Low, "économiseur d'Android");
        let plugged = Conditions {
            charging: true,
            power_save: true,
            ..on(false, 5).unwrap()
        };
        assert_eq!(p.pick(Some(plugged)), Lossless, "rien à ménager en charge");
        let unknown = Conditions {
            battery: None,
            ..on(false, 0).unwrap()
        };
        assert_eq!(
            p.pick(Some(unknown)),
            Lossless,
            "niveau inconnu : pas de plafond"
        );
    }

    #[test]
    fn conditions_from_kotlin() {
        let c: Conditions = serde_json::from_str(
            r#"{"metered":true,"battery":42,"charging":false,"powerSave":true}"#,
        )
        .unwrap();
        assert!(c.metered && c.power_save && !c.charging);
        assert_eq!(c.battery, Some(42));
        let c: Conditions = serde_json::from_str(r#"{"metered":false,"powerSave":false}"#).unwrap();
        assert_eq!(c.battery, None);
    }

    #[test]
    fn ceiling() {
        let p = policy(High, Lossless, true);
        let expected = if cfg!(mobile) {
            AudioQuality::Lossless
        } else {
            AudioQuality::HiResLossless
        };
        assert_eq!(p.ceiling(), expected);
    }
}
