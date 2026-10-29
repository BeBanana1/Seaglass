//! Sauvegarde (au plus toutes les 2 s) et restauration de la file et de la position.

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::actor::Player;

const PERSIST_INTERVAL: Duration = Duration::from_secs(2);

impl Player {
    /// Restaure la file en pause ; le titre n'est chargé dans mpv qu'à la reprise.
    pub(super) fn restore(&mut self) {
        let Some(store) = &self.store else {
            return;
        };
        let saved = match store.load_player() {
            Ok(Some(saved)) => saved,
            Ok(None) => return,
            Err(e) => {
                tracing::warn!("restauration de la file impossible : {e}");
                return;
            }
        };
        self.tidal.cache.insert_all(&saved.tracks);
        self.queue = saved.queue;
        self.state.track = self.queue.current().and_then(|id| self.tidal.cache.get(id));
        self.state.time_pos = saved.position;
        self.state.duration = self
            .state
            .track
            .as_ref()
            .map_or(0.0, |t| f64::from(t.duration));
        self.state.paused = true;
        self.queue_dirty = true;
        tracing::info!(
            "file restaurée : titre {:?} à {:.0} s, {} métadonnées",
            self.queue.current().map(|id| id.0),
            saved.position,
            saved.tracks.len()
        );
    }

    pub(super) fn persist_if_due(&mut self) {
        if !(self.persist_queue || self.persist_position)
            || self.last_persist.elapsed() < PERSIST_INTERVAL
        {
            return;
        }
        let Some(store) = self.store.as_ref().map(Arc::clone) else {
            return;
        };
        let queue = self.persist_queue.then(|| {
            let tracks = self.tidal.cache.get_many(self.queue.referenced_ids());
            (self.queue.clone(), tracks)
        });
        let position = self.state.time_pos;
        self.persist_queue = false;
        self.persist_position = false;
        self.last_persist = Instant::now();

        tokio::task::spawn_blocking(move || {
            let queue = queue.as_ref().map(|(q, t)| (q, t.as_slice()));
            if let Err(e) = store.save_player(queue, position) {
                tracing::warn!("sauvegarde de la file impossible : {e}");
            }
        });
    }
}
