//! Relance du moteur après un arrêt inattendu (#255) : mpv planté, ou figé puis arrêté par
//! le délai des commandes (`mpv/engine.rs`).

use std::time::{Duration, Instant};

use super::actor::Player;
use super::settings::PlaybackSettings;
use crate::engine;
use crate::i18n::tr;
use crate::mpv::MpvIssue;

/// Une relance au plus par période : un mpv qui retombe aussitôt ne tourne pas en boucle,
/// le bandeau prend le relais.
const RESTART_COOLDOWN: Duration = Duration::from_secs(5 * 60);

fn restart_allowed(last: Option<Instant>, now: Instant) -> bool {
    last.is_none_or(|last| now.duration_since(last) >= RESTART_COOLDOWN)
}

impl Player {
    pub(super) async fn on_engine_stopped(&mut self) {
        let issue = engine::diagnose().unwrap_or(MpvIssue::Stopped);
        let was_playing = self.loaded && !self.state.paused;
        self.engine = Err(issue.message().into());
        self.loaded = false;
        self.next_loaded = None;
        self.preloading = false;
        self.preload_req += 1;
        // Fermeture de l'app ou mise à jour : arrêt voulu, rien à signaler.
        if !self.launch.may_restart() {
            return;
        }
        let now = Instant::now();
        // AVX2 absent ou mpv manquant : une relance échouerait pareil (#92, #261).
        if issue == MpvIssue::Stopped && restart_allowed(self.last_restart, now) {
            self.last_restart = Some(now);
            tracing::warn!("mpv arrêté, relance");
            let cache = PlaybackSettings::load(self.store.as_deref()).cache_size;
            let started = self
                .launch
                .start(cache, self.output.launch_device(), self.events_tx.clone())
                .await;
            match started {
                Ok(engine) => {
                    self.engine = Ok(engine);
                    self.state.mpv_issue = None;
                    self.apply_loop_file().await;
                    self.apply_volume().await;
                    // En pause, le titre sera chargé au prochain Lecture, comme après une
                    // restauration.
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
                Err(e) => tracing::error!("relance de mpv impossible : {e}"),
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
