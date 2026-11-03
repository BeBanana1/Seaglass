//! Sauvegarde (au plus toutes les 2 s) et restauration de la file, de la position et du
//! volume.

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::actor::Player;
use crate::tidal::models::TrackId;

const PERSIST_INTERVAL: Duration = Duration::from_secs(2);

impl Player {
    /// Restaure la file en pause ; le titre n'est chargé dans mpv qu'à la reprise.
    pub(super) fn restore(&mut self) {
        let Some(store) = &self.store else {
            return;
        };
        match store.load_volume() {
            Ok(Some(volume)) => self.state.volume = volume,
            Ok(None) => {}
            Err(e) => tracing::warn!("restauration du volume impossible : {e}"),
        }
        let saved = match store.load_player() {
            Ok(Some(saved)) => saved,
            Ok(None) => return,
            Err(e) => {
                tracing::warn!("restauration de la file impossible : {e}");
                return;
            }
        };
        self.queue = saved.queue;
        self.pin_queue_tracks();
        self.tidal.cache.insert_all(&saved.tracks);
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
        if !(self.persist_queue || self.persist_position || self.persist_volume)
            || self.last_persist.elapsed() < PERSIST_INTERVAL
        {
            return;
        }
        let Some(store) = self.store.as_ref().map(Arc::clone) else {
            return;
        };
        // Seule la file est copiée : le store ne demande au cache que les titres qui ne
        // sont pas encore en base (#90).
        let queue = self.persist_queue.then(|| self.queue.clone());
        let tidal = Arc::clone(&self.tidal);
        let position = self.state.time_pos;
        let volume = self.persist_volume.then_some(self.state.volume);
        self.persist_queue = false;
        self.persist_position = false;
        self.persist_volume = false;
        self.last_persist = Instant::now();

        tokio::task::spawn_blocking(move || {
            let lookup = |ids: &[TrackId]| tidal.cache.get_many(ids.iter().copied());
            if let Err(e) = store.save_player(queue.as_ref(), position, lookup) {
                tracing::warn!("sauvegarde de la file impossible : {e}");
            }
            if let Some(volume) = volume
                && let Err(e) = store.save_volume(volume)
            {
                tracing::warn!("sauvegarde du volume impossible : {e}");
            }
        });
    }
}
