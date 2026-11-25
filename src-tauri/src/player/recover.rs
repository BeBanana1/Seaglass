//! Restart of the engine after an unexpected stop (#255): mpv crashed, or froze and was
//! stopped by the command timeout (`mpv/engine.rs`).

use std::time::{Duration, Instant};

use super::actor::Player;
use super::settings::PlaybackSettings;
use crate::engine;
use crate::i18n::tr;
use crate::mpv::MpvIssue;

/// One restart per period at most: an mpv that falls again at once does not loop,
/// the banner takes over.
const RESTART_COOLDOWN: Duration = Duration::from_secs(5 * 60);

fn restart_allowed(last: Option<Instant>, now: Instant) -> bool {
    last.is_none_or(|last| now.duration_since(last) >= RESTART_COOLDOWN)
}

impl Player {
    pub(super) async fn on_engine_stopped(&mut self) {
        let issue = engine::diagnose().unwrap_or(MpvIssue::Stopped);
        let was_playing = self.loaded && !self.state.paused;
        self.log_close();
        self.engine = Err(issue.message().into());
        self.loaded = false;
        self.next_loaded = None;
        self.preloading = false;
        self.preload_req += 1;
        // App closing or update: intended stop, nothing to report.
        if !self.launch.may_restart() {
            return;
        }
        let now = Instant::now();
        // No AVX2 or mpv missing: a restart would fail the same way (#92, #261).
        if issue == MpvIssue::Stopped && restart_allowed(self.last_restart, now) {
            self.last_restart = Some(now);
            tracing::warn!("mpv stopped, restarting");
            let cache = PlaybackSettings::load(self.store.as_deref()).cache_size;
            let started = self
                .launch
                .start(
                    cache,
                    self.output.launch_device(),
                    self.state.exclusive,
                    self.events_tx.clone(),
                )
                .await;
            match started {
                Ok(engine) => {
                    self.engine = Ok(engine);
                    self.state.mpv_issue = None;
                    self.apply_loop_file().await;
                    self.apply_volume().await;
                    // Paused: the track is loaded on the next Play, as after a
                    // restore.
                    if was_playing && let Some(id) = self.queue.current() {
                        self.load_current(id, self.state.time_pos);
                    }
                    self.toast(
                        "info",
                        tr(
                            "mpv s'est arrêté, il a été relancé",
                            "mpv stopped and was restarted",
                        )
                        .into(),
                    );
                    return;
                }
                Err(e) => tracing::error!("cannot restart mpv: {e}"),
            }
        }
        self.state.mpv_issue = Some(issue);
        self.toast("error", issue.message().into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restarts_at_most_once_per_cooldown() {
        let now = Instant::now();
        assert!(restart_allowed(None, now));
        assert!(!restart_allowed(Some(now), now + Duration::from_secs(10)));
        assert!(restart_allowed(Some(now), now + RESTART_COOLDOWN));
    }
}
