//! Sleep timer (#85): pause after a duration, at the end of the current track or at the
//! end of the queue (album, playlist… and manual queue, without looping or radio, #436).
//! Not persisted: a restart cancels it.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::actor::Player;
use super::queue::Repeat;
use crate::i18n::trf;

/// Maximum accepted duration (the UI offers 15 to 90 min).
const MAX_MINUTES: u16 = 600;

/// Request from the UI.
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

/// Active timer, in `PlayerState::sleep`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export, export_to = "../../src/lib/bindings/")
)]
pub enum SleepState {
    /// Deadline in ms since the UNIX epoch: the UI computes the remaining time itself,
    /// without a `player-state` every second.
    Minutes {
        #[serde(rename = "endsAt")]
        #[cfg_attr(test, ts(type = "number"))]
        ends_at: u64,
    },
    EndOfTrack,
    /// Pause after the last track of the queue (#436): becomes `EndOfTrack` once the queue
    /// reaches its end (`Queue::at_end`), or at once in repeat One (the track loops).
    EndOfContext,
}

impl Player {
    /// The current track is the last one before the pause: nothing must follow it gaplessly.
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
            // The preloaded next one (back to the start in repeat All, radio) would follow
            // gaplessly: it is removed, and `maybe_preload` loads nothing while the timer
            // targets the end of the track.
            self.invalidate_next().await;
        }
        if end_of_track != was_end_of_track {
            // Repeat One loops with `loop-file`: lifted while the timer is active.
            self.apply_loop_file().await;
        }
    }

    /// Called by the actor's one-second tick.
    pub(super) async fn check_sleep_timer(&mut self) {
        if self.sleep_deadline.is_some_and(|d| Instant::now() >= d) {
            tracing::info!("sleep timer: pause");
            self.sleep_deadline = None;
            self.state.sleep = None;
            self.set_paused(true).await;
        }
    }

    /// Natural end of the last track before the pause: the queue advances without loading
    /// the next one, paused, like a queue restored at startup (Play will load it).
    /// In repeat One, the same track stays, at the start.
    pub(super) async fn sleep_after_track(&mut self) {
        tracing::info!("sleep timer: end of the track or of the queue");
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
