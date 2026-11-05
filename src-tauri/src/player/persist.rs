//! Sauvegarde (au plus toutes les 2 s) et restauration de la file, de la position et du
//! volume ; écoutes de l'historique local (#202).

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::actor::Player;
use crate::store::Play;
use crate::tidal::models::{AlbumSummary, Track, TrackId};

const PERSIST_INTERVAL: Duration = Duration::from_secs(2);
/// Position seule pendant la lecture : sur Android, une écriture toutes les 10 s suffit
/// (des heures d'écoute écran éteint, #147). En pause, la cadence normale reprend pour que
/// la position soit gardée si le système tue l'app.
#[cfg(desktop)]
const POSITION_INTERVAL: Duration = PERSIST_INTERVAL;
#[cfg(mobile)]
const POSITION_INTERVAL: Duration = Duration::from_secs(10);

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
        let only_position = !(self.persist_queue || self.persist_volume);
        let interval = if only_position && !self.state.paused {
            POSITION_INTERVAL
        } else {
            PERSIST_INTERVAL
        };
        if !(self.persist_queue || self.persist_position || self.persist_volume)
            || self.last_persist.elapsed() < interval
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

    /// Position reçue : l'écoute est enregistrée une fois le seuil franchi (`listen.rs`),
    /// hors de l'acteur.
    pub(super) fn note_listen(&mut self) {
        let Some(track) = &self.state.track else {
            return;
        };
        let duration = if self.state.duration > 0.0 {
            self.state.duration
        } else {
            f64::from(track.duration)
        };
        if self
            .listen
            .on_position(track.id, duration, self.state.time_pos)
            .is_none()
        {
            return;
        }
        let Some(store) = self.store.clone() else {
            return;
        };
        let album = album_of(track);
        let source = self.queue.source().cloned();
        let id = track.id;
        let played_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        tokio::task::spawn_blocking(move || {
            let play = Play {
                track: id,
                album: Some(&album),
                source: source.as_ref(),
                played_at,
            };
            if let Err(e) = store.add_play(&play) {
                tracing::warn!("écoute non enregistrée : {e}");
            }
        });
    }
}

/// Carte d'album tirée du titre : de quoi l'afficher sans requête. Les artistes sont ceux du
/// titre (le plus souvent ceux de l'album).
fn album_of(track: &Track) -> AlbumSummary {
    let album = &track.album;
    AlbumSummary {
        id: album.id,
        title: album.title.clone(),
        artists: track.artists.clone(),
        cover_url: album.cover_url.clone(),
        cover_large_url: album.cover_large_url.clone(),
        color: album.color.clone(),
        release_date: None,
        number_of_tracks: None,
        album_type: None,
        explicit: track.explicit,
        hires: track.hires,
    }
}
