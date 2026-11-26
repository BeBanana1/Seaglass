//! Transport controls of the player: next, previous, pause, seek, Repeat One, volume and
//! cache size, applied to the engine when a track is loaded.

use super::actor::Player;
use super::queue::Repeat;
use crate::engine::Op;
use crate::mpv::CacheSize;

/// Below this, Previous goes to the previous track rather than the start of this one.
const RESTART_THRESHOLD: f64 = 3.0;

impl Player {
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
