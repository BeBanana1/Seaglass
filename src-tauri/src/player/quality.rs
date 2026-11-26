//! Quality requested from TIDAL. Desktop: always Hi-Res Lossless. Android (#325, #365,
//! #366): the setting for the current network (metered or not), capped when the battery is
//! low. Network and battery are reread each time a track is resolved: a change counts from
//! the next track, the current track is never reloaded.

use serde::{Deserialize, Serialize};

use super::settings::{MobileQuality, PlaybackSettings};
use crate::tidal::models::AudioQuality;

// Only read on Android (and in tests): the desktop does not choose its quality.
/// Below this battery level (in %), "High" at most.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
const HIGH_CAP_BELOW: u8 = 50;
/// Below this level, or with Android's battery saver on, "Low" at most.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
const LOW_CAP_BELOW: u8 = 20;

/// Rules taken from the playback settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct QualityPolicy {
    wifi: MobileQuality,
    cellular: MobileQuality,
    low_battery: bool,
}

/// Device state read by `DeviceConditions.kt`.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(super) struct Conditions {
    /// Metered network (mobile data, tethering) or no network.
    pub metered: bool,
    /// Level in %, if known.
    pub battery: Option<u8>,
    /// Plugged in, even with a full battery.
    pub charging: bool,
    /// Android's battery saver.
    pub power_save: bool,
}

/// Quality lowered by the battery (#492), sent to the UI.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityCap {
    /// Quality requested for now.
    pub quality: MobileQuality,
    /// The setting's for the current network.
    pub chosen: MobileQuality,
    pub reason: CapReason,
    /// Level in %, if known.
    pub battery: Option<u8>,
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CapReason {
    /// Android's battery saver.
    PowerSave,
    /// Below 50% (High at most) or below 20% (Low).
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

    /// Best quality possible with these settings: a hi-res edition is only looked up if it
    /// can be played.
    pub fn ceiling(self) -> AudioQuality {
        if cfg!(mobile) {
            self.wifi.max(self.cellular).audio_quality()
        } else {
            AudioQuality::HiResLossless
        }
    }

    /// Quality for this device state. Without a state (unreadable), the lower of the two
    /// network qualities: better not to use data by mistake. The battery has the last word
    /// over the network.
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

    /// Quality chosen for the current network.
    fn network(self, c: Conditions) -> MobileQuality {
        if c.metered { self.cellular } else { self.wifi }
    }

    /// Battery cap that lowers the current network's quality (#492), to show it in the
    /// settings. `None` if the battery changes nothing: setting off, charging, battery high
    /// enough, or chosen quality already under the cap.
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

    /// Quality to request for the track being resolved now.
    pub async fn current(self) -> AudioQuality {
        #[cfg(target_os = "android")]
        {
            let conditions = read_conditions().await;
            let quality = self.pick(conditions);
            tracing::debug!("quality {quality:?} ({conditions:?})");
            quality.audio_quality()
        }
        #[cfg(not(target_os = "android"))]
        {
            self.ceiling()
        }
    }
}

/// Battery cap of the moment for these settings (#492); `None` if the device state is
/// unreadable.
#[cfg(target_os = "android")]
pub async fn current_cap(settings: &PlaybackSettings) -> Option<QualityCap> {
    let conditions = read_conditions().await?;
    QualityPolicy::new(settings).cap(conditions)
}

/// Network and battery read by `DeviceConditions.kt`.
#[cfg(target_os = "android")]
async fn read_conditions() -> Option<Conditions> {
    tauri_plugin_seaglass_mobile::direct::call::<Conditions>(
        "deviceConditions",
        serde_json::json!({}),
    )
    .await
    .inspect_err(|e| tracing::warn!("network and battery state unreadable: {e}"))
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
        assert_eq!(p.pick(on(true, 80)), High, "mobile data");
        assert_eq!(
            p.pick(on(true, 5)),
            High,
            "battery ignored without the setting"
        );
        assert_eq!(p.pick(None), High, "unknown state: the lower of the two");
    }

    #[test]
    fn low_battery_caps_the_network_quality() {
        let p = policy(Max, Lossless, true);
        assert_eq!(p.pick(on(false, 50)), Max);
        assert_eq!(p.pick(on(false, 49)), High);
        assert_eq!(p.pick(on(true, 49)), High);
        assert_eq!(p.pick(on(false, 20)), High);
        assert_eq!(p.pick(on(false, 19)), Low);
        // The cap never raises a lower chosen quality.
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
        assert_eq!(p.pick(Some(saver)), Low, "Android battery saver");
        let plugged = Conditions {
            charging: true,
            power_save: true,
            ..on(false, 5).unwrap()
        };
        assert_eq!(
            p.pick(Some(plugged)),
            Lossless,
            "nothing to spare while charging"
        );
        let unknown = Conditions {
            battery: None,
            ..on(false, 0).unwrap()
        };
        assert_eq!(p.pick(Some(unknown)), Lossless, "unknown level: no cap");
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
            "battery saver: Low instead of Max"
        );
        let cap = p.cap(on(false, 40).unwrap()).unwrap();
        assert_eq!((cap.quality, cap.reason), (High, CapReason::BatteryLow));
        assert_eq!(p.cap(on(false, 10).unwrap()).unwrap().quality, Low);
        assert_eq!(p.cap(on(false, 60).unwrap()), None, "batterie assez haute");
        assert_eq!(
            p.cap(on(true, 40).unwrap()),
            None,
            "mobile data already on High: nothing lowered"
        );
        let plugged = Conditions {
            charging: true,
            ..saver
        };
        assert_eq!(p.cap(plugged), None, "en charge");
        assert_eq!(policy(Max, High, false).cap(saver), None, "setting off");
        // `pick` follows the cap.
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
