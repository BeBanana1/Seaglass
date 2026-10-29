//! État du lecteur tel qu'envoyé à l'UI (événement `player-state`).

use serde::Serialize;

use crate::mpv::AudioParams;
use crate::tidal::models::{StreamFormat, Track};

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    pub track: Option<Track>,
    /// Format servi par TIDAL.
    pub stream: Option<StreamFormat>,
    /// Format réellement envoyé à la sortie audio par mpv.
    pub output: Option<AudioParams>,
    pub output_driver: Option<String>,
    /// Un titre est en cours de résolution (métadonnées + manifeste).
    pub loading: bool,
    pub paused: bool,
    pub buffering: bool,
    pub idle: bool,
    /// Position en secondes ; l'UI interpole entre deux événements.
    pub time_pos: f64,
    pub duration: f64,
}

impl PlayerState {
    /// Vrai si seuls `time_pos` diffère : ces changements sont limités à ~4 Hz.
    pub fn differs_only_by_time(&self, other: &Self) -> bool {
        self.time_pos != other.time_pos
            && Self {
                time_pos: other.time_pos,
                ..self.clone()
            } == *other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_only_difference() {
        let a = PlayerState::default();
        let b = PlayerState {
            time_pos: 1.0,
            ..a.clone()
        };
        let c = PlayerState {
            time_pos: 1.0,
            paused: true,
            ..a.clone()
        };
        assert!(a.differs_only_by_time(&b));
        assert!(!a.differs_only_by_time(&c));
        assert!(!a.differs_only_by_time(&a));
    }
}
