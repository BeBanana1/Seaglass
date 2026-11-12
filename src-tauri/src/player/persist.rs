//! Sauvegarde (au plus toutes les 2 s) et restauration de la file, de la position et du
//! volume ; écoutes de l'historique local (#202).

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use super::actor::{Cmd, Player};
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

/// Délai maximal d'une sauvegarde de fermeture : au-delà, l'app quitte sans l'attendre.
const FLUSH_TIMEOUT: Duration = Duration::from_secs(1);

/// Demande à l'acteur une sauvegarde immédiate et l'attend `FLUSH_TIMEOUT` au plus (#260).
/// Bloquant, sans runtime tokio : appelable depuis `RunEvent::Exit` comme depuis l'updater.
#[cfg_attr(mobile, allow(dead_code))]
pub fn flush_blocking(player: &mpsc::Sender<Cmd>) {
    let (reply, done) = std::sync::mpsc::channel();
    if player.try_send(Cmd::Flush(reply)).is_err() {
        tracing::warn!("sauvegarde de fermeture non demandée : lecteur indisponible");
        return;
    }
    if done.recv_timeout(FLUSH_TIMEOUT).is_err() {
        tracing::warn!("sauvegarde de fermeture non terminée après {FLUSH_TIMEOUT:?}");
    }
}

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

    /// Reprise au lancement (#353) : attend la session TIDAL hors de l'acteur, puis lui
    /// demande de lire le titre restauré. Sans session (déconnecté, réseau absent), rien.
    pub(super) fn resume_when_signed_in(&self) {
        if self.queue.current().is_none() {
            return;
        }
        let tidal = Arc::clone(&self.tidal);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            match tidal.current_session().await {
                Ok(Some(_)) => {
                    let _ = tx.send(Cmd::ResumeOnLaunch).await;
                }
                Ok(None) => tracing::info!("pas de reprise au lancement : aucune session"),
                Err(e) => tracing::warn!("pas de reprise au lancement : {e}"),
            }
        });
    }

    /// Lit le titre restauré à sa position, sauf si l'utilisateur a déjà lancé quelque
    /// chose (`load_req`) ou si le titre était presque fini.
    pub(super) fn resume_on_launch(&mut self) {
        if self.loaded || self.load_req != 0 || !self.state.paused {
            return;
        }
        let Some(id) = self.queue.current() else {
            return;
        };
        if !worth_resuming(self.state.time_pos, self.state.duration) {
            tracing::info!("pas de reprise au lancement : titre presque fini");
            return;
        }
        tracing::info!("reprise de la lecture au lancement");
        self.load_current(id, self.state.time_pos);
    }

    pub(super) fn persist_if_due(&mut self) {
        let only_position = !(self.persist_queue || self.persist_volume || self.persist_profiles);
        let interval = if only_position && !self.state.paused {
            POSITION_INTERVAL
        } else {
            PERSIST_INTERVAL
        };
        if !(self.persist_queue
            || self.persist_position
            || self.persist_volume
            || self.persist_profiles)
            || self.last_persist.elapsed() < interval
        {
            return;
        }
        self.last_write = self.write().map(tokio::task::spawn_blocking);
    }

    /// Sauvegarde de fermeture (#260) : tout ce qui est en attente, et la position même
    /// inchangée. Attend l'écriture précédente puis la sienne avant de répondre.
    pub(super) fn flush(&mut self, reply: std::sync::mpsc::Sender<()>) {
        if self.state.track.is_some() {
            self.persist_position = true;
        }
        let previous = self.last_write.take();
        let write = self.write();
        tokio::spawn(async move {
            if let Some(previous) = previous {
                let _ = previous.await;
            }
            if let Some(write) = write {
                let _ = tokio::task::spawn_blocking(write).await;
            }
            let _ = reply.send(());
        });
    }

    /// Écriture de ce qui est marqué à sauvegarder, à lancer hors de l'acteur ; `None` sans
    /// store ou sans rien à écrire.
    fn write(&mut self) -> Option<impl FnOnce() + Send + 'static> {
        if !(self.persist_queue
            || self.persist_position
            || self.persist_volume
            || self.persist_profiles)
        {
            return None;
        }
        let store = self.store.as_ref().map(Arc::clone)?;
        // Seule la file est copiée : le store ne demande au cache que les titres qui ne
        // sont pas encore en base (#90).
        let queue = self.persist_queue.then(|| self.queue.clone());
        let tidal = Arc::clone(&self.tidal);
        let position = self.state.time_pos;
        let volume = self.persist_volume.then_some(self.state.volume);
        let profiles = self
            .persist_profiles
            .then(|| self.profiles.entries().to_vec());
        self.persist_queue = false;
        self.persist_profiles = false;
        self.persist_position = false;
        self.persist_volume = false;
        self.last_persist = Instant::now();

        Some(move || {
            let lookup = |ids: &[TrackId]| tidal.cache.get_many(ids.iter().copied());
            if let Err(e) = store.save_player(queue.as_ref(), position, lookup) {
                tracing::warn!("sauvegarde de la file impossible : {e}");
            }
            if let Some(volume) = volume
                && let Err(e) = store.save_volume(volume)
            {
                tracing::warn!("sauvegarde du volume impossible : {e}");
            }
            if let Some(profiles) = profiles {
                super::profiles::save(&store, &profiles);
            }
        })
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
        let track = track.clone();
        let played_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        tokio::task::spawn_blocking(move || {
            let play = Play {
                track: &track,
                album: &album,
                source: source.as_ref(),
                played_at,
            };
            if let Err(e) = store.add_play(&play) {
                tracing::warn!("écoute non enregistrée : {e}");
            }
        });
    }
}

/// Fin de titre trop proche pour une reprise au lancement (#353).
const RESUME_MARGIN: f64 = 5.0;

/// Une reprise au lancement vaut la peine s'il reste plus de `RESUME_MARGIN` secondes ;
/// durée inconnue : on reprend.
fn worth_resuming(position: f64, duration: f64) -> bool {
    duration <= 0.0 || duration - position > RESUME_MARGIN
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
        ai: track.ai,
        hires: track.hires,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_skips_a_track_about_to_end() {
        assert!(worth_resuming(0.0, 200.0));
        assert!(worth_resuming(120.0, 200.0));
        assert!(!worth_resuming(196.0, 200.0));
        assert!(!worth_resuming(200.0, 200.0));
        assert!(worth_resuming(30.0, 0.0), "durée inconnue : on reprend");
    }
}
