//! Engine window of at most 2 entries: current track (0) + preloaded next one (1).
//!
//! Stream URLs are signed and expire: nothing further is ever loaded.

use super::actor::{Cmd, Player, Slot};
use super::queue::Repeat;
use super::settings::HiresUpgrade;
use super::upgrade;
use crate::engine::{End, Engine, Op};
use crate::i18n::{tr, trf};
use crate::mpv::CacheSize;
use crate::tidal::models::{AudioQuality, Stream, StreamSource, StreamUse, Track, TrackId};

/// The next track is preloaded when less than this is left on the current one.
const PRELOAD_BEFORE_END: f64 = 30.0;
/// Below this, Previous goes to the previous track rather than the start of this one.
const RESTART_THRESHOLD: f64 = 3.0;
/// Consecutive failures past which playback stops moving on.
const MAX_FAILURES: u32 = 3;

impl Player {
    /// Loads `id` as entry 0 (replaces the whole engine window) from `start`.
    pub(super) fn load_current(&mut self, id: TrackId, start: f64) {
        if let Err(e) = &self.engine {
            let message = e.clone();
            self.toast("error", message);
            return;
        }
        if self.retried != Some(id) {
            self.retried = None;
        }
        self.log_close();
        self.load_req += 1;
        self.preload_req += 1;
        self.preloading = false;
        self.next_loaded = None;
        self.loaded = false;
        self.state.loading = true;
        self.state.ended = false;
        self.state.track = self.tidal.cache.get(id);
        self.state.stream = None;
        self.state.hires_offer = None;
        self.state.normalization = None;
        self.offer_req += 1;
        self.state.time_pos = start;
        self.state.cached = 0.0;
        self.resolve(id, Slot::Current { start }, self.load_req);
    }

    /// Preloads the next track under 30 s from the end (except repeat One: `loop-file`, and
    /// the "End of track" sleep timer: nothing must follow gaplessly; blind test: each round
    /// is loaded alone, #434).
    pub(super) fn maybe_preload(&mut self) {
        let remaining = self.state.duration - self.state.time_pos;
        if self.next_loaded.is_some()
            || self.blind.is_some()
            || self.preloading
            || self.state.duration <= 0.0
            || remaining >= PRELOAD_BEFORE_END
            || self.queue.repeat() == Repeat::One
            || self.sleeps_at_end_of_track()
        {
            return;
        }
        let Some(id) = self.queue.peek_next() else {
            // End of the queue: the radio, if enabled, is preloaded when it arrives.
            return self.maybe_request_radio();
        };
        self.preload_req += 1;
        self.preloading = true;
        self.resolve(id, Slot::Next(id), self.preload_req);
    }

    /// Removes the preloaded track from the engine and cancels a preload in progress.
    pub(super) async fn invalidate_next(&mut self) {
        self.preload_req += 1;
        self.preloading = false;
        if self.next_loaded.take().is_some() {
            self.engine_op(Op::RemoveNext).await;
        }
    }

    /// Resolves metadata and stream outside the actor, which stays responsive meanwhile.
    /// The stream may come from another hi-res edition (`upgrade::resolve_stream`): the queue
    /// and the display keep `id`. In "Ask" mode, only for an album whose offer was accepted.
    fn resolve(&self, id: TrackId, slot: Slot, request: u64) {
        let (tidal, tx) = (self.tidal.clone(), self.tx.clone());
        let (mode, policy) = (self.hires_upgrade, self.quality);
        let accepted: Vec<u64> = self
            .hires_answers
            .iter()
            .filter_map(|(album, yes)| yes.then_some(*album))
            .collect();
        let current = matches!(slot, Slot::Current { .. });
        tauri::async_runtime::spawn(async move {
            let result = async {
                // On Android, reads the current network and battery (#365).
                let quality = policy.current().await;
                // Without a possible hi-res edition (Android, "No" setting), the stream does
                // not depend on the metadata: both requests go out together (#148).
                if quality != AudioQuality::HiResLossless || mode == HiresUpgrade::Off {
                    let use_ = if current {
                        StreamUse::Play
                    } else {
                        StreamUse::Preload
                    };
                    let (track, stream) =
                        tokio::join!(tidal.track(id), tidal.stream(id, quality, use_));
                    return Ok(Box::new((track?, stream?)));
                }
                let track = tidal.track(id).await?;
                let upgrade = match mode {
                    HiresUpgrade::Auto => true,
                    HiresUpgrade::Ask => accepted.contains(&track.album.id),
                    HiresUpgrade::Off => false,
                };
                let stream =
                    upgrade::resolve_stream(&tidal, &track, quality, upgrade, current).await?;
                Ok::<_, crate::tidal::TidalError>(Box::new((track, stream)))
            }
            .await
            .map_err(|e| e.to_string());
            let _ = tx
                .send(Cmd::Resolved {
                    request,
                    slot,
                    result,
                })
                .await;
        });
    }

    pub(super) async fn on_resolved(
        &mut self,
        request: u64,
        slot: Slot,
        result: Result<Box<(Track, Stream)>, String>,
    ) {
        match slot {
            Slot::Current { start } if request == self.load_req => {
                self.state.loading = false;
                let (track, stream) = match result {
                    Ok(resolved) => *resolved,
                    Err(e) => return self.on_track_failed(e).await,
                };
                let Some(url) = self.stream_url(&track, &stream) else {
                    return;
                };
                log_stream("playing", &track, &stream);
                // Normalization gain as a file option (#82): it does not touch the track
                // preloaded afterwards.
                let gain = self.normalizer.loaded(stream.gain);
                self.state.normalization = gain;
                let filters = self.filters(gain);
                self.engine_op(Op::Play {
                    url,
                    start,
                    filters,
                })
                .await;
                self.loaded = true;
                self.log_loaded(stream.session, track.album.id, start);
                // The engine emits nothing if it was not paused: the state is set here.
                self.state.paused = false;
                self.state.track = Some(track);
                self.state.stream = Some(stream.format);
                self.maybe_offer_hires();
            }
            Slot::Next(id) if request == self.preload_req => {
                self.preloading = false;
                match result {
                    Ok(resolved) => {
                        let (track, stream) = *resolved;
                        let Some(url) = self.stream_url(&track, &stream) else {
                            return;
                        };
                        log_stream("preloading", &track, &stream);
                        let gain = self.normalizer.preloaded(stream.gain);
                        let filters = self.filters(gain);
                        self.engine_op(Op::Preload { url, filters }).await;
                        self.next_loaded = Some((track, stream.format, stream.session));
                    }
                    // Without a preload, the end of the track triggers a regular load.
                    Err(e) => tracing::warn!("cannot preload {id}: {e}"),
                }
            }
            _ => {} // Resolution superseded by a newer one.
        }
    }

    /// The engine moved to the preloaded entry: the queue advances and the old entry is
    /// removed.
    pub(super) async fn on_advanced(&mut self) {
        let Some((track, format, session)) = self.next_loaded.take() else {
            return;
        };
        let advanced = self.queue.advance();
        self.mark_queue_dirty();
        self.engine_op(Op::DropPrevious).await;
        if advanced == Some(track.id) {
            // The engine may already report the next track's duration: the previous one ends
            // at its own.
            let previous = self
                .state
                .track
                .as_ref()
                .map_or(self.state.duration, |t| f64::from(t.duration));
            self.log_advanced(session, track.album.id, previous);
            self.retried = None;
            self.state.track = Some(track);
            self.state.stream = Some(format);
            self.state.normalization = self.normalizer.advance();
            self.state.time_pos = 0.0;
            self.maybe_offer_hires();
        } else if let Some(id) = advanced {
            // Should not happen: any change to the queue removes entry 1.
            tracing::warn!("preloaded {} but the queue gives {id}", track.id);
            self.load_current(id, 0.0);
        }
    }

    pub(super) async fn on_end_file(&mut self, end: End) {
        match end {
            // Natural end with nothing preloaded (no preload, or it failed).
            End::Eof if self.loaded && self.next_loaded.is_none() => {
                self.log_ended();
                self.loaded = false;
                if self.sleeps_at_end_of_track() {
                    self.sleep_after_track().await;
                } else if let Some(id) = self.queue.advance() {
                    self.mark_queue_dirty();
                    self.load_current(id, 0.0);
                } else {
                    self.on_queue_end();
                }
            }
            End::Eof => {}
            // The chosen output device does not open: fallback, not a stream error.
            End::OutputFailed => {
                // Exclusive: no silent fallback to shared mode or another device.
                if self.state.exclusive {
                    self.on_exclusive_failed().await;
                } else if !self.on_output_init_failed().await {
                    let detail = tr(
                        "ouverture de la sortie audio impossible",
                        "audio output initialization failed",
                    );
                    self.on_playback_error(Some(detail.into())).await;
                }
            }
            End::Error(error) => self.on_playback_error(error).await,
        }
    }

    /// Engine error: a new URL once (expired signature?), then the next track.
    async fn on_playback_error(&mut self, error: Option<String>) {
        let Some(id) = self.queue.current() else {
            return;
        };
        let detail = error.unwrap_or_else(|| tr("erreur inconnue", "unknown error").into());
        if self.retried != Some(id) {
            tracing::warn!("playback error on {id} ({detail}), new URL");
            self.retried = Some(id);
            self.load_current(id, self.state.time_pos);
        } else {
            self.on_track_failed(trf!(
                "lecture impossible : {detail}",
                "cannot play: {detail}"
            ))
            .await;
        }
    }

    /// The current track cannot be played: toast, then the next track.
    async fn on_track_failed(&mut self, message: String) {
        self.log_close();
        self.toast_kind("error", Some("unplayable"), message);
        self.failures += 1;
        self.loaded = false;
        if self.failures >= MAX_FAILURES {
            // Blind test (#434): the game stops too, the earlier queue comes back.
            self.end_blind_test().await;
            self.failures = 0;
            self.toast_kind(
                "error",
                Some("stopped"),
                tr(
                    "trop d'échecs consécutifs, lecture arrêtée",
                    "too many consecutive failures, playback stopped",
                )
                .into(),
            );
            self.engine_op(Op::Stop).await;
            return;
        }
        if self.blind_track_failed().await {
            return;
        }
        if let Some(id) = self.queue.advance() {
            self.mark_queue_dirty();
            self.load_current(id, 0.0);
        }
    }

    pub(super) async fn skip_next(&mut self) {
        if let Some(id) = self.queue.advance() {
            self.on_queue_changed().await;
            self.load_current(id, 0.0);
        }
    }

    /// Past 3 s: back to the start of the track; otherwise the previous track of the
    /// history. Either way playback resumes, even from a pause (#47).
    pub(super) async fn previous(&mut self) {
        match previous_action(self.state.time_pos, self.queue.has_previous()) {
            Previous::Restart => {
                self.seek(0.0).await;
                // Loads the track at 0 when it is not in the engine (restored, ended).
                self.set_paused(false).await;
            }
            Previous::Track => {
                if let Some(id) = self.queue.previous() {
                    self.on_queue_changed().await;
                    self.load_current(id, 0.0);
                }
            }
        }
    }

    /// After a restore, the track is not in the engine yet: it is loaded at the saved
    /// position.
    pub(super) async fn toggle_pause(&mut self) {
        if self.loaded {
            self.engine_op(Op::TogglePause).await;
        } else if let Some(id) = self.queue.current() {
            self.load_current(id, self.state.time_pos);
        }
    }

    /// Media keys: Play starts a restored track, Pause does nothing when nothing is
    /// loaded.
    pub(super) async fn set_paused(&mut self, paused: bool) {
        if self.loaded {
            self.engine_op(Op::SetPaused(paused)).await;
        } else if !paused && let Some(id) = self.queue.current() {
            self.load_current(id, self.state.time_pos);
        }
    }

    pub(super) async fn seek(&mut self, secs: f64) {
        if self.loaded {
            self.log_seek(secs);
            self.engine_op(Op::Seek(secs)).await;
        } else {
            self.state.time_pos = secs;
            self.persist_position = true;
        }
    }

    /// Repeat One = the track loops in the engine (`loop-file=inf` with mpv), without a
    /// reload; lifted while the sleep timer targets the end of the track (#85).
    pub(super) async fn apply_loop_file(&mut self) {
        if self.engine.is_err() {
            return;
        }
        let on = self.queue.repeat() == Repeat::One && !self.sleeps_at_end_of_track();
        self.engine_op(Op::LoopCurrent(on)).await;
    }

    pub(super) async fn on_volume_changed(&mut self) {
        self.persist_volume = true;
        self.note_device_profile();
        self.apply_volume().await;
    }

    /// Engine software volume (mpv: cubic curve, 100 = unity gain) and mute.
    pub(super) async fn apply_volume(&mut self) {
        if self.engine.is_err() {
            return;
        }
        let v = self.state.volume;
        self.engine_op(Op::Volume {
            level: v.level,
            muted: v.muted,
        })
        .await;
    }

    /// New cache size: mpv also applies it to the demuxer already open, without a reload,
    /// as soon as playback consumes packets (checked by the test
    /// `mpv::cache::tests::live_cache_resize`). Without mpv, it is used at the next launch
    /// (read from the settings).
    pub(super) async fn apply_cache_size(&mut self, size: CacheSize) {
        if self.engine.is_err() {
            return;
        }
        self.engine_op(Op::CacheSize(size)).await;
    }

    fn stream_url(&self, track: &Track, stream: &Stream) -> Option<String> {
        // Android: the MPD goes as a `data:` URL, without a local server.
        #[cfg(mobile)]
        if let StreamSource::Dash(mpd) = &stream.source {
            return Some(crate::media3::data_url(mpd));
        }
        match (&stream.source, &self.server) {
            (StreamSource::Direct(url), _) => Some(url.clone()),
            (StreamSource::Dash(mpd), Some(server)) => {
                Some(server.publish(&track.id.to_string(), mpd.clone()))
            }
            (StreamSource::Dash(_), None) => {
                self.toast(
                    "error",
                    tr(
                        "serveur de manifestes indisponible",
                        "manifest server unavailable",
                    )
                    .into(),
                );
                None
            }
        }
    }

    /// Sends an operation to the engine; an error (missing engine included) becomes a toast.
    pub(super) async fn engine_op(&mut self, op: Op) {
        let result = match &self.engine {
            Ok(engine) => engine.apply(op).await,
            Err(e) => Err(e.clone()),
        };
        if let Err(e) = result {
            self.toast("error", e);
        }
    }
}

fn log_stream(what: &str, track: &Track, stream: &Stream) {
    let f = &stream.format;
    tracing::info!(
        "{what} {} : {:?} {} {:?}/{:?}",
        track.id,
        f.quality,
        f.codec,
        f.bit_depth,
        f.sample_rate
    );
}

/// What Previous does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Previous {
    /// Back to the start of the current track.
    Restart,
    /// The previous track of the history.
    Track,
}

fn previous_action(time_pos: f64, has_previous: bool) -> Previous {
    if time_pos > RESTART_THRESHOLD || !has_previous {
        Previous::Restart
    } else {
        Previous::Track
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previous_restarts_past_the_threshold_or_without_history() {
        assert_eq!(previous_action(3.5, true), Previous::Restart);
        assert_eq!(previous_action(120.0, true), Previous::Restart);
        assert_eq!(previous_action(0.0, false), Previous::Restart);
        assert_eq!(previous_action(2.0, false), Previous::Restart);
    }

    #[test]
    fn previous_goes_back_a_track_under_the_threshold() {
        assert_eq!(previous_action(0.0, true), Previous::Track);
        assert_eq!(previous_action(RESTART_THRESHOLD, true), Previous::Track);
    }
}
