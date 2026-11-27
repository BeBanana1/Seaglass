//! Automatic radio at the end of the queue (#78, `PlaybackSettings::auto_radio` setting).
//!
//! Requested at preload time, when nothing follows the current track: the radio arrives
//! before the end of the track and its first track is preloaded like any other (gapless
//! kept). The queue logic is in `Queue::extend_with_radio`.

use super::actor::{Cmd, Player};
use crate::tidal::models::{RadioSeed, TrackId};

#[derive(Debug, Default)]
pub(super) struct AutoRadio {
    pub(super) enabled: bool,
    /// Counter against stale responses.
    req: u64,
    /// Track whose radio was already requested: a single request per track, even on failure,
    /// as long as the queue does not change (`Player::mark_queue_dirty` clears it).
    pub(super) asked: Option<TrackId>,
    pending: bool,
    /// Playback stopped at the end of the queue before the response: it resumes on the radio.
    waiting: bool,
}

impl AutoRadio {
    pub(super) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ..Self::default()
        }
    }
}

impl Player {
    /// Nothing follows the current track: requests its radio, outside the actor.
    pub(super) fn maybe_request_radio(&mut self) {
        let Some(seed) = self.queue.current() else {
            return;
        };
        if !self.radio.enabled || !self.queue.wants_radio() || self.radio.asked == Some(seed) {
            return;
        }
        self.radio.asked = Some(seed);
        self.radio.req += 1;
        self.radio.pending = true;
        let (tidal, tx, request) = (self.tidal.clone(), self.tx.clone(), self.radio.req);
        tauri::async_runtime::spawn(async move {
            let result = tidal
                .radio(RadioSeed::Track, seed.0)
                .await
                .map(|page| {
                    // The content filter (#368) also applies to the automatic radio.
                    crate::content::filtered(page.tracks)
                        .into_iter()
                        .map(|t| t.id)
                        .collect()
                })
                .map_err(|e| e.to_string());
            let _ = tx
                .send(Cmd::RadioResolved {
                    request,
                    seed,
                    result,
                })
                .await;
        });
    }

    /// End of the queue reached (last track finished, nothing preloaded): playback will
    /// resume when the radio arrives, if it is on its way.
    pub(super) fn on_queue_end(&mut self) {
        // Blind test (#434): the revealed track played to the end, the next round waits.
        if self.blind.is_some() {
            return;
        }
        self.maybe_request_radio();
        self.radio.waiting = self.radio.pending;
        // Without a radio on its way, the queue is really over ("End" card, #399).
        self.state.ended = !self.radio.waiting;
    }

    pub(super) async fn on_radio_resolved(
        &mut self,
        request: u64,
        seed: TrackId,
        result: Result<Vec<TrackId>, String>,
    ) {
        if request != self.radio.req {
            return;
        }
        self.radio.pending = false;
        let waiting = std::mem::take(&mut self.radio.waiting);
        let tracks = match result {
            Ok(tracks) => tracks,
            Err(e) => {
                // The queue was waiting for this radio: it stops there.
                self.state.ended |= waiting;
                return tracing::warn!("radio de {seed} indisponible : {e}");
            }
        };
        if !self.radio.enabled || !self.queue.extend_with_radio(seed, &tracks) {
            self.state.ended |= waiting;
            return;
        }
        tracing::info!("queue extended by the radio of {seed}");
        self.mark_queue_dirty();
        if waiting {
            if let Some(id) = self.queue.advance() {
                self.load_current(id, 0.0);
            }
        } else {
            self.maybe_preload();
        }
    }
}
