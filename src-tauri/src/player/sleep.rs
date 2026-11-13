//! Minuteur de mise en veille (#85) : pause après une durée, à la fin du titre en cours ou à
//! la fin de la file (album, playlist… et file manuelle, sans boucler ni radio, #436).
//! Non persisté : un redémarrage l'annule.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::actor::Player;
use super::queue::Repeat;
use crate::i18n::trf;

/// Durée maximale acceptée (l'UI propose 15 à 90 min).
const MAX_MINUTES: u16 = 600;

/// Demande de l'UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SleepRequest {
    Minutes { minutes: u16 },
    EndOfTrack,
    EndOfContext,
}

impl SleepRequest {
    pub fn validate(self) -> Result<Self, String> {
        match self {
            Self::Minutes { minutes } if minutes == 0 || minutes > MAX_MINUTES => Err(trf!(
                "durée du minuteur invalide : {minutes} min (1 à {MAX_MINUTES})",
                "invalid sleep timer duration: {minutes} min (1 to {MAX_MINUTES})"
            )),
            _ => Ok(self),
        }
    }
}

/// Minuteur actif, dans `PlayerState::sleep`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SleepState {
    /// Échéance en ms depuis l'epoch UNIX : l'UI calcule le temps restant elle-même,
    /// sans un `player-state` par seconde.
    Minutes {
        #[serde(rename = "endsAt")]
        ends_at: u64,
    },
    EndOfTrack,
    /// Pause après le dernier titre de la file (#436) : se ramène à `EndOfTrack` une fois la
    /// file au bout (`Queue::at_end`), ou tout de suite en repeat One (le titre boucle).
    EndOfContext,
}

impl Player {
    /// Le titre en cours est le dernier avant la pause : rien ne doit le suivre en gapless.
    pub(super) fn sleeps_at_end_of_track(&self) -> bool {
        match self.state.sleep {
            Some(SleepState::EndOfTrack) => true,
            Some(SleepState::EndOfContext) => {
                self.queue.repeat() == Repeat::One || self.queue.at_end()
            }
            _ => false,
        }
    }

    pub(super) async fn set_sleep_timer(&mut self, request: Option<SleepRequest>) {
        let was_end_of_track = self.sleeps_at_end_of_track();
        self.sleep_deadline = None;
        self.state.sleep = request.map(|r| match r {
            SleepRequest::Minutes { minutes } => {
                let delay = Duration::from_secs(u64::from(minutes) * 60);
                self.sleep_deadline = Some(Instant::now() + delay);
                SleepState::Minutes {
                    ends_at: epoch_ms(SystemTime::now() + delay),
                }
            }
            SleepRequest::EndOfTrack => SleepState::EndOfTrack,
            SleepRequest::EndOfContext => SleepState::EndOfContext,
        });
        let end_of_track = self.sleeps_at_end_of_track();
        if end_of_track {
            // Le suivant préchargé (retour au début en repeat All, radio) serait enchaîné en
            // gapless : il est retiré, et `maybe_preload` ne recharge rien tant que le
            // minuteur vise la fin du titre.
            self.invalidate_next().await;
        }
        if end_of_track != was_end_of_track {
            // Repeat One boucle avec `loop-file` : levé tant que le minuteur est actif.
            self.apply_loop_file().await;
        }
    }

    /// Appelé par le tick d'une seconde de l'acteur.
    pub(super) async fn check_sleep_timer(&mut self) {
        if self.sleep_deadline.is_some_and(|d| Instant::now() >= d) {
            tracing::info!("minuteur de mise en veille : pause");
            self.sleep_deadline = None;
            self.state.sleep = None;
            self.set_paused(true).await;
        }
    }

    /// Fin naturelle du dernier titre avant la pause : la file avance sans charger le
    /// suivant, en pause, comme une file restaurée au démarrage (Lecture le chargera).
    /// En repeat One, le même titre reste, au début.
    pub(super) async fn sleep_after_track(&mut self) {
        tracing::info!("minuteur de mise en veille : fin du titre ou de la file");
        self.state.sleep = None;
        if self.queue.repeat() == Repeat::One {
            self.apply_loop_file().await;
        } else if self.queue.advance().is_some() {
            self.mark_queue_dirty();
        }
        self.state.track = self.queue.current().and_then(|id| self.tidal.cache.get(id));
        self.state.duration = self
            .state
            .track
            .as_ref()
            .map_or(0.0, |t| f64::from(t.duration));
        self.state.time_pos = 0.0;
        self.state.stream = None;
        self.state.hires_offer = None;
        self.state.paused = true;
        self.persist_position = true;
    }
}

fn epoch_ms(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_is_read_and_validated() {
        let read =
            |json: &str| serde_json::from_str::<SleepRequest>(json).map(SleepRequest::validate);
        assert_eq!(
            read(r#"{"kind":"minutes","minutes":30}"#).unwrap(),
            Ok(SleepRequest::Minutes { minutes: 30 })
        );
        assert_eq!(
            read(r#"{"kind":"endOfTrack"}"#).unwrap(),
            Ok(SleepRequest::EndOfTrack)
        );
        assert_eq!(
            read(r#"{"kind":"endOfContext"}"#).unwrap(),
            Ok(SleepRequest::EndOfContext)
        );
        assert!(read(r#"{"kind":"minutes","minutes":0}"#).unwrap().is_err());
        assert!(
            read(r#"{"kind":"minutes","minutes":601}"#)
                .unwrap()
                .is_err()
        );
        assert!(read(r#"{"kind":"minutes","minutes":-5}"#).is_err());
        assert!(read(r#"{"kind":"hours","hours":1}"#).is_err());
    }

    #[test]
    fn state_is_serialized_for_the_ui() {
        let json = |s: SleepState| serde_json::to_string(&s).unwrap();
        assert_eq!(
            json(SleepState::Minutes { ends_at: 1_000 }),
            r#"{"kind":"minutes","endsAt":1000}"#
        );
        assert_eq!(json(SleepState::EndOfTrack), r#"{"kind":"endOfTrack"}"#);
        assert_eq!(json(SleepState::EndOfContext), r#"{"kind":"endOfContext"}"#);
    }

    #[test]
    fn epoch_ms_counts_from_unix_epoch() {
        assert_eq!(epoch_ms(UNIX_EPOCH + Duration::from_millis(1_500)), 1_500);
        assert_eq!(epoch_ms(UNIX_EPOCH - Duration::from_secs(1)), 0);
    }
}
