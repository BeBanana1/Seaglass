//! Saving (at most every 2 s) and restoring of the queue, position and volume; listens of
//! the local history (#202).

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use super::actor::{Cmd, Player};
use crate::integrations::listenbrainz;
use crate::store::Play;
use crate::tidal::models::{AlbumSummary, Track, TrackId};

const PERSIST_INTERVAL: Duration = Duration::from_secs(2);
/// Position alone during playback: on Android, a write every 10 s is enough (hours of
/// listening with the screen off, #147). When paused, the normal pace resumes so that the
/// position is kept if the system kills the app.
#[cfg(desktop)]
const POSITION_INTERVAL: Duration = PERSIST_INTERVAL;
#[cfg(mobile)]
const POSITION_INTERVAL: Duration = Duration::from_secs(10);

/// Maximum delay of a shutdown save: beyond it, the app quits without waiting for it.
const FLUSH_TIMEOUT: Duration = Duration::from_secs(1);

/// Asks the actor for an immediate save and waits for it `FLUSH_TIMEOUT` at most (#260).
/// Blocking, without a tokio runtime: callable from `RunEvent::Exit` as from the updater.
#[cfg_attr(mobile, allow(dead_code))]
pub fn flush_blocking(player: &mpsc::Sender<Cmd>) {
    let (reply, done) = std::sync::mpsc::channel();
    if player.try_send(Cmd::Flush(reply)).is_err() {
        tracing::warn!("shutdown save not requested: player unavailable");
        return;
    }
    if done.recv_timeout(FLUSH_TIMEOUT).is_err() {
        tracing::warn!("shutdown save not finished after {FLUSH_TIMEOUT:?}");
    }
}

impl Player {
    /// Restores the queue paused; the track is only loaded into mpv on resume.
    pub(super) fn restore(&mut self) {
        let Some(store) = &self.store else {
            return;
        };
        match store.load_volume() {
            Ok(Some(volume)) => self.state.volume = volume,
            Ok(None) => {}
            Err(e) => tracing::warn!("cannot restore the volume: {e}"),
        }
        let saved = match store.load_player() {
            Ok(Some(saved)) => saved,
            Ok(None) => return,
            Err(e) => {
                tracing::warn!("cannot restore the queue: {e}");
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
            "queue restored: track {:?} at {:.0} s, {} metadata entries",
            self.queue.current().map(|id| id.0),
            saved.position,
            saved.tracks.len()
        );
    }

    /// Resume at launch (#353): waits for the TIDAL session outside the actor, then asks it
    /// to play the restored track. Without a session (signed out, no network), nothing.
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
                Ok(None) => tracing::info!("no resume at launch: no session"),
                Err(e) => tracing::warn!("no resume at launch: {e}"),
            }
        });
    }

    /// Plays the restored track at its position, unless the user already started something
    /// (`load_req`) or the track was almost over.
    pub(super) fn resume_on_launch(&mut self) {
        if self.loaded || self.load_req != 0 || !self.state.paused {
            return;
        }
        let Some(id) = self.queue.current() else {
            return;
        };
        if !worth_resuming(self.state.time_pos, self.state.duration) {
            tracing::info!("no resume at launch: track almost over");
            return;
        }
        tracing::info!("resuming playback at launch");
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

    /// Shutdown save (#260): everything pending, and the position even if unchanged. Waits
    /// for the previous write, then its own, before answering.
    pub(super) fn flush(&mut self, reply: std::sync::mpsc::Sender<()>) {
        if self.state.track.is_some() {
            self.persist_position = true;
        }
        let previous = self.last_write.take();
        let write = self.write();
        // The current listen ends here and is queued for TIDAL's play log (#34).
        let playlog = self.log_close_for_exit();
        tokio::spawn(async move {
            if let Some(previous) = previous {
                let _ = previous.await;
            }
            if let Some(write) = write {
                let _ = tokio::task::spawn_blocking(write).await;
            }
            if let Some(playlog) = playlog {
                playlog.await;
            }
            let _ = reply.send(());
        });
    }

    /// Write of what is marked to be saved, to be run outside the actor; `None` without a
    /// store or with nothing to write.
    fn write(&mut self) -> Option<impl FnOnce() + Send + 'static> {
        if !(self.persist_queue
            || self.persist_position
            || self.persist_volume
            || self.persist_profiles)
        {
            return None;
        }
        let store = self.store.as_ref().map(Arc::clone)?;
        // Only the queue is copied: the store only asks the cache for the tracks that are not
        // in the database yet (#90).
        // During a blind test (#434), the queue and position from before the game.
        let queue = self.persist_queue.then(|| self.main_queue().clone());
        let tidal = Arc::clone(&self.tidal);
        let position = self.main_position();
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
                tracing::warn!("cannot save the queue: {e}");
            }
            if let Some(volume) = volume
                && let Err(e) = store.save_volume(volume)
            {
                tracing::warn!("cannot save the volume: {e}");
            }
            if let Some(profiles) = profiles {
                super::profiles::save(&store, &profiles);
            }
        })
    }

    /// Position received: the listen is recorded once the threshold is crossed
    /// (`listen.rs`), outside the actor.
    /// Blind test clips (#434) do not count.
    pub(super) fn note_listen(&mut self) {
        if self.blind.is_some() {
            return;
        }
        let Some(track) = &self.state.track else {
            return;
        };
        let duration = if self.state.duration > 0.0 {
            self.state.duration
        } else {
            f64::from(track.duration)
        };
        let counted = self
            .listen
            .on_position(track.id, duration, self.state.time_pos);
        // Start of the track announced to ListenBrainz (#431) once it really plays.
        if !self.state.paused && self.listen.announce(track.id) {
            listenbrainz::now_playing(track);
        }
        if counted.is_none() {
            return;
        }
        let Some(store) = self.store.clone() else {
            return;
        };
        let album = album_of(track);
        let source = self.queue.source().cloned();
        let track = track.clone();
        let format = self.state.played_format();
        let played_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        // ListenBrainz wants the start of the listen, frozen here: a repeated send keeps the
        // same date.
        let started_at = played_at / 1000 - self.listen.listened() as i64;
        tokio::task::spawn_blocking(move || {
            let play = Play {
                track: &track,
                album: &album,
                source: source.as_ref(),
                played_at,
                format: format.as_ref(),
            };
            if let Err(e) = store.add_play(&play) {
                tracing::warn!("listen not recorded: {e}");
            }
            listenbrainz::listened(&store, &track, started_at);
        });
    }
}

/// End of track too close for a resume at launch (#353).
const RESUME_MARGIN: f64 = 5.0;

/// A resume at launch is worth it if more than `RESUME_MARGIN` seconds remain; unknown
/// duration: resume.
fn worth_resuming(position: f64, duration: f64) -> bool {
    duration <= 0.0 || duration - position > RESUME_MARGIN
}

/// Album card derived from the track: enough to display it without a request. The artists
/// are the track's (most often the album's).
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
        assert!(worth_resuming(30.0, 0.0), "unknown duration: resume");
    }
}
