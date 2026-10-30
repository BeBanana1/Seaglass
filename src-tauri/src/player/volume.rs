//! Volume du lecteur : niveau 0–100 transmis tel quel à la propriété mpv `volume`.
//!
//! Pas de courbe côté app : mpv applique déjà un gain cubique (`(volume / 100)³`), donc
//! perceptif, et 100 correspond exactement au gain unitaire. Tout autre niveau (ou la
//! coupure) est un gain logiciel : la sortie n'est plus bit-perfect.

use serde::{Deserialize, Serialize};

/// Niveau maximal, aussi plafonné par `volume-max=100` dans `seaglass.conf`.
pub const MAX_LEVEL: u8 = 100;
/// Niveau repris quand on rétablit le son alors que le curseur est à 0.
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
    /// Vérifie un niveau reçu de l'UI.
    pub fn validate(level: u8) -> Result<u8, InvalidVolume> {
        if level > MAX_LEVEL {
            Err(InvalidVolume(level))
        } else {
            Ok(level)
        }
    }

    /// Change le niveau ; bouger le curseur rétablit le son coupé (comme les autres
    /// lecteurs), sauf à 0.
    pub fn set_level(&mut self, level: u8) {
        self.level = level.min(MAX_LEVEL);
        if self.level > 0 {
            self.muted = false;
        }
    }

    /// Coupe ou rétablit le son. Rétablir à 0 remonterait à un silence : le niveau repart
    /// alors de `UNMUTE_LEVEL`.
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        if !muted && self.level == 0 {
            self.level = UNMUTE_LEVEL;
        }
    }

    /// Valeur relue depuis la base : un niveau hors limites est ramené à 100.
    pub fn sanitized(self) -> Self {
        Self {
            level: self.level.min(MAX_LEVEL),
            ..self
        }
    }

    /// Vrai si mpv ne modifie pas l'amplitude (niveau 100, son non coupé).
    #[cfg(test)]
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
}
