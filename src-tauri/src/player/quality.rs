//! Qualité demandée à TIDAL. Bureau : toujours Hi-Res Lossless. Android (#325, #365, #366) :
//! le réglage du réseau en cours (facturé à l'usage ou non), plafonné quand la batterie est
//! faible. Réseau et batterie sont relus à chaque résolution d'un titre : un changement
//! compte à partir du titre suivant, le titre en cours n'est jamais rechargé.

use serde::{Deserialize, Serialize};

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

/// Qualité baissée par la batterie (#492), envoyée à l'UI.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityCap {
    /// Qualité demandée pour l'instant.
    pub quality: MobileQuality,
    /// Celle du réglage pour le réseau en cours.
    pub chosen: MobileQuality,
    pub reason: CapReason,
    /// Niveau en %, s'il est connu.
    pub battery: Option<u8>,
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CapReason {
    /// Économiseur de batterie d'Android.
    PowerSave,
    /// Sous 50 % (Haute au plus) ou sous 20 % (Basse).
    BatteryLow,
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
        match self.cap(c) {
            Some(cap) => cap.quality,
            None => self.network(c),
        }
    }

    /// Qualité choisie pour le réseau en cours.
    fn network(self, c: Conditions) -> MobileQuality {
        if c.metered { self.cellular } else { self.wifi }
    }

    /// Plafond de batterie qui baisse la qualité du réseau en cours (#492), pour le montrer
    /// dans les réglages. `None` si la batterie ne change rien : réglage coupé, en charge,
    /// batterie assez haute, ou qualité choisie déjà sous le plafond.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn cap(self, c: Conditions) -> Option<QualityCap> {
        if !self.low_battery || c.charging {
            return None;
        }
        let below = |threshold: u8| c.battery.is_some_and(|level| level < threshold);
        let (limit, reason) = if c.power_save {
            (MobileQuality::Low, CapReason::PowerSave)
        } else if below(LOW_CAP_BELOW) {
            (MobileQuality::Low, CapReason::BatteryLow)
        } else if below(HIGH_CAP_BELOW) {
            (MobileQuality::High, CapReason::BatteryLow)
        } else {
            return None;
        };
        let chosen = self.network(c);
        (limit < chosen).then_some(QualityCap {
            quality: limit,
            chosen,
            reason,
            battery: c.battery,
        })
    }

    /// Qualité à demander pour le titre résolu maintenant.
    pub async fn current(self) -> AudioQuality {
        #[cfg(target_os = "android")]
        {
            let conditions = read_conditions().await;
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

/// Plafond de batterie de l'instant pour ces réglages (#492) ; `None` si l'état de
/// l'appareil est illisible.
#[cfg(target_os = "android")]
pub async fn current_cap(settings: &PlaybackSettings) -> Option<QualityCap> {
    let conditions = read_conditions().await?;
    QualityPolicy::new(settings).cap(conditions)
}

/// Réseau et batterie lus par `DeviceConditions.kt`.
#[cfg(target_os = "android")]
async fn read_conditions() -> Option<Conditions> {
    tauri_plugin_seaglass_mobile::direct::call::<Conditions>(
        "deviceConditions",
        serde_json::json!({}),
    )
    .await
    .inspect_err(|e| tracing::warn!("état du réseau et de la batterie illisible : {e}"))
    .ok()
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
    fn cap_explains_only_a_lowered_quality() {
        let p = policy(Max, High, true);
        let saver = Conditions {
            power_save: true,
            ..on(false, 90).unwrap()
        };
        assert_eq!(
            p.cap(saver),
            Some(QualityCap {
                quality: Low,
                chosen: Max,
                reason: CapReason::PowerSave,
                battery: Some(90),
            }),
            "économiseur : Basse au lieu de Max"
        );
        let cap = p.cap(on(false, 40).unwrap()).unwrap();
        assert_eq!((cap.quality, cap.reason), (High, CapReason::BatteryLow));
        assert_eq!(p.cap(on(false, 10).unwrap()).unwrap().quality, Low);
        assert_eq!(p.cap(on(false, 60).unwrap()), None, "batterie assez haute");
        assert_eq!(
            p.cap(on(true, 40).unwrap()),
            None,
            "données mobiles déjà en Haute : rien de baissé"
        );
        let plugged = Conditions {
            charging: true,
            ..saver
        };
        assert_eq!(p.cap(plugged), None, "en charge");
        assert_eq!(policy(Max, High, false).cap(saver), None, "réglage coupé");
        // `pick` suit le plafond.
        assert_eq!(p.pick(Some(saver)), Low);
        assert_eq!(p.pick(on(true, 40)), High);
    }

    #[test]
    fn cap_for_the_ui() {
        let cap = policy(Max, Max, true)
            .cap(Conditions {
                power_save: true,
                ..on(false, 61).unwrap()
            })
            .unwrap();
        assert_eq!(
            serde_json::to_string(&cap).unwrap(),
            r#"{"quality":"low","chosen":"max","reason":"powerSave","battery":61}"#
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
