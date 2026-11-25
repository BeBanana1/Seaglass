//! Player volume: level 0–100 passed as is to the mpv `volume` property.
//!
//! No curve on the app side: mpv already applies a cubic gain (`(volume / 100)³`), so
//! perceptual, and 100 is exactly unity gain. Any other level (or mute) is a software
//! gain: the output is no longer bit-perfect.

use serde::{Deserialize, Serialize};

/// Maximum level, also capped by `volume-max=100` in `seaglass.conf`.
pub const MAX_LEVEL: u8 = 100;
/// Level restored when unmuting while the slider is at 0.
const UNMUTE_LEVEL: u8 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{}", crate::i18n::trf!(
    "volume invalide : {} (attendu entre 0 et {MAX_LEVEL})",
    "invalid volume: {} (expected 0 to {MAX_LEVEL})",
    .0
))]
pub struct InvalidVolume(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export, export_to = "../../src/lib/bindings/")
)]
pub struct Volume {
    pub level: u8,
    pub muted: bool,
}

impl Default for Volume {
    fn default() -> Self {
        Self {
            level: MAX_LEVEL,
            muted: false,
        }
    }
}

impl Volume {
    /// Checks a level received from the UI.
    pub fn validate(level: u8) -> Result<u8, InvalidVolume> {
        if level > MAX_LEVEL {
            Err(InvalidVolume(level))
        } else {
            Ok(level)
        }
    }

    /// Changes the level; moving the slider unmutes (like other players), except to
    /// 0.
    pub fn set_level(&mut self, level: u8) {
        self.level = level.min(MAX_LEVEL);
        if self.level > 0 {
            self.muted = false;
        }
    }

    /// One step up or down (global shortcuts, #413), from the level heard: 0 when
    /// muted. Lowering a muted sound changes nothing.
    #[cfg(any(desktop, test))]
    pub fn step(&mut self, delta: i8) {
        if self.muted && delta < 0 {
            return;
        }
        let base = if self.muted { 0 } else { i16::from(self.level) };
        let next = (base + i16::from(delta)).clamp(0, i16::from(MAX_LEVEL));
        self.set_level(u8::try_from(next).unwrap_or(MAX_LEVEL));
    }

    /// Mutes or unmutes. Unmuting at 0 would bring back silence: the level then restarts
    /// from `UNMUTE_LEVEL`.
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        if !muted && self.level == 0 {
            self.level = UNMUTE_LEVEL;
        }
    }

    /// Value read back from the database: an out-of-range level is brought back to 100.
    pub fn sanitized(self) -> Self {
        Self {
            level: self.level.min(MAX_LEVEL),
            ..self
        }
    }

    /// True when mpv leaves the amplitude untouched (level 100, not muted).
    pub fn is_unity(self) -> bool {
        self.level == MAX_LEVEL && !self.muted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_unity_gain() {
        assert!(Volume::default().is_unity());
    }

    #[test]
    fn validate_bounds() {
        assert_eq!(Volume::validate(0), Ok(0));
        assert_eq!(Volume::validate(100), Ok(100));
        assert_eq!(Volume::validate(101), Err(InvalidVolume(101)));
        assert_eq!(Volume::validate(255), Err(InvalidVolume(255)));
    }

    #[test]
    fn moving_the_slider_unmutes() {
        let mut v = Volume {
            level: 40,
            muted: true,
        };
        v.set_level(0);
        assert!(v.muted, "à 0, la coupure reste");
        v.set_level(60);
        assert_eq!(
            v,
            Volume {
                level: 60,
                muted: false
            }
        );
        assert!(!v.is_unity());
        v.set_level(200);
        assert!(v.is_unity());
    }

    #[test]
    fn unmuting_at_zero_restores_a_level() {
        let mut v = Volume {
            level: 70,
            muted: false,
        };
        v.set_muted(true);
        v.set_muted(false);
        assert_eq!(
            v,
            Volume {
                level: 70,
                muted: false
            }
        );

        v.set_level(0);
        v.set_muted(true);
        v.set_muted(false);
        assert_eq!(v.level, UNMUTE_LEVEL);
        assert!(!v.muted);
    }

    #[test]
    fn sanitized_clamps_stored_level() {
        let v = Volume {
            level: 180,
            muted: true,
        };
        assert_eq!(
            v.sanitized(),
            Volume {
                level: 100,
                muted: true
            }
        );
        let ok = Volume {
            level: 30,
            muted: false,
        };
        assert_eq!(ok.sanitized(), ok);
    }

    #[test]
    fn serializes_for_the_ui() {
        let json = serde_json::to_string(&Volume::default()).unwrap();
        assert_eq!(json, r#"{"level":100,"muted":false}"#);
    }

    #[test]
    fn step_moves_from_what_is_heard() {
        let mut v = Volume {
            level: 98,
            muted: false,
        };
        v.step(5);
        assert_eq!(
            v,
            Volume {
                level: 100,
                muted: false
            }
        );
        v.step(-5);
        assert_eq!(v.level, 95);
        let mut low = Volume {
            level: 3,
            muted: false,
        };
        low.step(-5);
        assert_eq!(
            low,
            Volume {
                level: 0,
                muted: false
            }
        );
        // Muted: raising starts from 0 and unmutes, lowering changes nothing.
        let mut muted = Volume {
            level: 60,
            muted: true,
        };
        muted.step(-5);
        assert_eq!(
            muted,
            Volume {
                level: 60,
                muted: true
            }
        );
        muted.step(5);
        assert_eq!(
            muted,
            Volume {
                level: 5,
                muted: false
            }
        );
    }
}
