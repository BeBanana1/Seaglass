//! Routing of the player actor: a command (`Cmd`) or an engine event (`Event`) to the
//! `Player` method that handles it.

use rand::Rng;

use super::actor::{Cmd, Player};
use super::blind;
use super::quality::QualityPolicy;
use crate::engine::Event;

impl Player {
    pub(super) async fn handle_cmd(&mut self, cmd: Cmd) {
        self.note_claim(&cmd);
        let cmd = match self.blind_route(cmd).await {
            Some(cmd) => cmd,
            None => return,
        };
        match cmd {
            Cmd::PlayTracks {
                ids,
                start,
                shuffle,
                source,
            } => {
                if let Some(on) = shuffle {
                    self.queue.set_shuffle(on, &mut self.rng);
                }
                let start = if shuffle == Some(true) && !ids.is_empty() {
                    self.rng.random_range(0..ids.len())
                } else {
                    start
                };
                let started = self.queue.set_context(ids, start, source, &mut self.rng);
                if let Some(id) = started {
                    self.on_queue_changed().await;
                    self.load_current(id, 0.0);
                }
            }
            Cmd::PlayNext(id) => {
                self.main_queue_mut().play_next(id);
                self.on_queue_changed().await;
            }
            Cmd::AddToQueue(id) => {
                self.main_queue_mut().add_to_queue(id);
                self.on_queue_changed().await;
            }
            Cmd::Enqueue { ids, next } => {
                if !ids.is_empty() {
                    self.main_queue_mut().enqueue(&ids, next);
                    self.on_queue_changed().await;
                }
            }
            Cmd::RemoveFromQueue(index) => {
                let before = self.main_queue().clone();
                if self.main_queue_mut().remove_manual(index) {
                    self.on_queue_changed().await;
                    self.undo_queue = Some(before);
                }
            }
            Cmd::RemoveFromQueueIds(ids) => {
                if self.main_queue_mut().remove_manual_ids(&ids) > 0 {
                    self.on_queue_changed().await;
                }
            }
            Cmd::MoveInQueue { from, to } => {
                // The preloaded track is always `peek_next`: reordering the middle of the
                // queue does not remove it from mpv (gapless intact, no new resolution).
                let next = self.main_queue().peek_next();
                if self.main_queue_mut().move_manual(from, to) {
                    if self.main_queue().peek_next() == next {
                        self.mark_queue_dirty();
                    } else {
                        self.on_queue_changed().await;
                    }
                }
            }
            Cmd::ClearQueue => {
                let before = self.main_queue().clone();
                if self.main_queue_mut().clear_upcoming() {
                    self.on_queue_changed().await;
                    self.undo_queue = Some(before);
                }
            }
            Cmd::RestoreQueue(reply) => {
                let restored = self.restore_queue().await;
                let _ = reply.send(restored);
            }
            Cmd::PlayFromQueue { part, index, id } => match self.queue.jump_to(part, index, id) {
                Some(id) => {
                    self.on_queue_changed().await;
                    self.load_current(id, 0.0);
                }
                None => tracing::debug!("queue changed since it was shown: track {} ignored", id.0),
            },
            Cmd::Next => self.skip_next().await,
            Cmd::Previous => self.previous().await,
            Cmd::TogglePause => self.toggle_pause().await,
            Cmd::SetPaused(paused) => self.set_paused(paused).await,
            Cmd::ResumeOnLaunch => self.resume_on_launch(),
            Cmd::AttachObservers(observers) => {
                observers.notify(&blind::public_state(&self.state, self.masks_track()));
                self.observers = *observers;
            }
            Cmd::Seek(secs) => self.seek(secs).await,
            Cmd::SetShuffle(on) => {
                let queue = match &mut self.blind {
                    Some(b) => &mut b.saved,
                    None => &mut self.queue,
                };
                queue.set_shuffle(on, &mut self.rng);
                self.on_queue_changed().await;
            }
            Cmd::SetRepeat(repeat) => {
                self.main_queue_mut().set_repeat(repeat);
                self.apply_loop_file().await;
                self.on_queue_changed().await;
            }
            Cmd::SetVolume(level) => {
                self.state.volume.set_level(level);
                self.on_volume_changed().await;
            }
            #[cfg(desktop)]
            Cmd::StepVolume(delta) => {
                self.state.volume.step(delta);
                self.on_volume_changed().await;
            }
            Cmd::SetMuted(muted) => {
                self.state.volume.set_muted(muted);
                self.on_volume_changed().await;
            }
            Cmd::SetPlaybackSettings(settings) => {
                // Applied at the next resolution: the current track is not reloaded, but
                // switching to "Ask" already offers its hi-res edition.
                if settings.hires_upgrade != self.hires_upgrade {
                    self.hires_upgrade = settings.hires_upgrade;
                    self.maybe_offer_hires();
                }
                // The preloaded next track may be in the old quality: it is requested
                // again.
                let quality = QualityPolicy::new(&settings);
                if quality != self.quality {
                    self.quality = quality;
                    self.invalidate_next().await;
                }
                self.radio.enabled = settings.auto_radio;
                self.pause_on_unplug = settings.pause_on_unplug;
                self.apply_cache_size(settings.cache_size).await;
                // Per-device profiles (#416): normalization and exclusive mode are noted
                // for the current output before a change of output applies the next
                // one's profile.
                self.set_device_profiles(settings.device_profiles);
                self.set_normalization(settings.normalization).await;
                self.set_equalizer(settings.equalizer).await;
                self.set_exclusive(settings.exclusive).await;
                self.set_audio_device(settings.audio_device).await;
            }
            Cmd::AnswerHiresOffer(accept) => self.answer_hires_offer(accept).await,
            Cmd::SetSleepTimer(request) => self.set_sleep_timer(request).await,
            Cmd::HiresOffer { request, offer } => {
                if request == self.offer_req {
                    self.state.hires_offer = Some(offer);
                }
            }
            Cmd::GetState(reply) => {
                let state = blind::public_state(&self.state, self.masks_track());
                let _ = reply.send(state.into_owned());
            }
            Cmd::Flush(reply) => self.flush(reply),
            Cmd::GetQueue(reply) => {
                let _ = reply.send(self.queue_view());
            }
            Cmd::TakeHandoff(reply) => {
                let handoff = self.take_handoff().await;
                let _ = reply.send(handoff);
            }
            Cmd::ApplyHandoff(handoff, reply) => {
                let applied = self.apply_handoff(*handoff).await;
                let _ = reply.send(applied);
            }
            Cmd::GetPlayOrder(limit, reply) => {
                let _ = reply.send(self.main_queue().play_order(limit));
            }
            Cmd::BlindTest(cmd) => self.blind_cmd(cmd).await,
            Cmd::GetAudioDevices(reply) => {
                let _ = reply.send(self.output.devices());
            }
            Cmd::Resolved {
                request,
                slot,
                result,
            } => self.on_resolved(request, slot, result).await,
            Cmd::RadioResolved {
                request,
                seed,
                result,
            } => self.on_radio_resolved(request, seed, result).await,
            Cmd::PlaybackTaken(taken) => self.on_playback_taken(taken).await,
        }
    }

    pub(super) async fn handle_event(&mut self, ev: Event) {
        let s = &mut self.state;
        match ev {
            // Position and duration only hold for a loaded track: before that, they would
            // overwrite the restored position or that of the track being resolved.
            Event::Position(t) if self.loaded => {
                s.time_pos = t.unwrap_or(0.0);
                self.persist_position = true;
                self.note_listen();
                self.log_position();
                self.maybe_preload();
                self.blind_on_position().await;
            }
            Event::Duration(d) if self.loaded => s.duration = d.unwrap_or(0.0),
            // After a gapless transition, the engine sends the new track's value: a drop is
            // always applied (`set_cached`).
            Event::Cached(t) if self.loaded => s.set_cached(t),
            Event::Position(_) | Event::Duration(_) | Event::Cached(_) => {}
            // With no track loaded, the pause stays that of the restored state.
            Event::Paused(p) if self.loaded => {
                s.paused = p;
                self.persist_position = true;
                self.log_paused(p);
            }
            Event::Paused(_) => {}
            Event::Buffering(b) => s.buffering = b,
            Event::Format(a) => s.set_output(a),
            Event::DeviceFormat(a) => s.set_device_format(a),
            Event::Driver(ao) => s.output_driver = ao,
            Event::Idle(i) => s.set_idle(i),
            Event::Advanced => self.on_advanced().await,
            Event::Devices(list) => self.on_device_list(list).await,
            Event::Loaded => {
                self.failures = 0;
                self.blind_on_loaded();
            }
            Event::Ended(end) => self.on_end_file(end).await,
            Event::Stopped => self.on_engine_stopped().await,
        }
    }

    /// Puts back the queue kept before a clear or a removal (#349), if the current track is
    /// still the same. Any other change to the queue has already dropped it.
    pub(super) async fn restore_queue(&mut self) -> bool {
        match self.undo_queue.take() {
            Some(before) if before.current() == self.main_queue().current() => {
                *self.main_queue_mut() = before;
                self.on_queue_changed().await;
                true
            }
            _ => false,
        }
    }
}
