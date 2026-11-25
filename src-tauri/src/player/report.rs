//! Sends TIDAL's play log (#34): each listen ended by `playlog.rs` is queued in SQLite
//! (`store::playlogs`) with the TIDAL user it was heard under, then a task posts the queue
//! in batches (`tidal::playlog`). Without network or session, it waits and retries with a
//! growing delay; a batch refused as a whole is retried one listen at a time, then the
//! culprit is dropped. Only counts and statuses are logged, never the form (it holds the
//! token); the event body, which holds none, at debug level.

use std::sync::{Arc, LazyLock};
use std::time::Duration;

use tokio::sync::Notify;

use super::actor::Player;
use super::source::PlaySource;
use crate::store::Store;
use crate::tidal::models::{LogSource, PlayLog, PlaybackSession};
use crate::tidal::playlog::{self, MAX_BATCH, Outgoing};
use crate::tidal::{TidalClient, TidalError};

const MIN_BACKOFF: Duration = Duration::from_secs(30);
const MAX_BACKOFF: Duration = Duration::from_secs(30 * 60);
/// Without a session, how often to look again (a sign-in does not wake the task).
const SESSION_POLL: Duration = Duration::from_secs(60);

/// A listen just entered the queue.
static WAKE: LazyLock<Notify> = LazyLock::new(Notify::new);

/// Unix time in ms.
pub(super) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// Starts the send task (the queue survives restarts: what is left goes out first).
pub fn start(tidal: Arc<TidalClient>, store: Option<Arc<Store>>) {
    let Some(store) = store else { return };
    tauri::async_runtime::spawn(run(tidal, store));
}

/// Queues ended listens under the current TIDAL user; nothing without a session. To be
/// awaited only where the queueing must be done (closing the app).
pub(super) async fn queue(tidal: &TidalClient, store: &Arc<Store>, logs: Vec<PlayLog>) {
    if logs.is_empty() {
        return;
    }
    let Some(user) = tidal.session().await.map(|s| s.user_id) else {
        tracing::debug!("play log: no session, {} listen(s) not queued", logs.len());
        return;
    };
    let now = now_ms();
    let events: Vec<(String, String)> = logs
        .iter()
        .map(|log| {
            let uuid = playlog::new_uuid();
            let body = playlog::message_body(log, &uuid, now);
            // No token in the body: readable with RUST_LOG=seaglass_lib::player::report=debug.
            tracing::debug!("play log queued: {body}");
            (uuid, body)
        })
        .collect();
    let store = Arc::clone(store);
    let queued = tokio::task::spawn_blocking(move || {
        for (uuid, body) in &events {
            store.queue_play_log(uuid, user, body)?;
        }
        Ok::<_, crate::store::StoreError>(())
    })
    .await;
    match queued {
        Ok(Ok(())) => WAKE.notify_one(),
        Ok(Err(e)) => tracing::warn!("play log: listen not queued: {e}"),
        Err(e) => tracing::warn!("play log: listen not queued: {e}"),
    }
}

async fn run(tidal: Arc<TidalClient>, store: Arc<Store>) {
    let mut batch = MAX_BATCH;
    let mut backoff = MIN_BACKOFF;
    loop {
        let Some(user) = tidal.session().await.map(|s| s.user_id) else {
            wait(SESSION_POLL).await;
            continue;
        };
        let read = Arc::clone(&store);
        let pending = match tokio::task::spawn_blocking(move || read.pending_play_logs(batch)).await
        {
            Ok(Ok(p)) => p,
            Ok(Err(e)) => {
                tracing::warn!("play log: queue unreadable: {e}");
                tokio::time::sleep(MAX_BACKOFF).await;
                continue;
            }
            Err(e) => {
                tracing::warn!("play log: queue unreadable: {e}");
                return;
            }
        };
        if pending.is_empty() {
            WAKE.notified().await;
            continue;
        }
        // Listens of another account can't go out under this token.
        let others: Vec<i64> = pending
            .iter()
            .filter(|l| l.user_id != user)
            .map(|l| l.id)
            .collect();
        if !others.is_empty() {
            tracing::info!(
                "play log: {} listen(s) of another account dropped",
                others.len()
            );
            drop_logs(&store, others).await;
            continue;
        }
        let events: Vec<Outgoing<'_>> = pending
            .iter()
            .map(|l| Outgoing {
                uuid: &l.uuid,
                body: &l.body,
            })
            .collect();
        match tidal.send_play_logs(&events, now_ms()).await {
            Ok(outcome) => {
                let done: Vec<i64> = pending
                    .iter()
                    .filter(|l| {
                        outcome.accepted.contains(&l.uuid) || outcome.malformed.contains(&l.uuid)
                    })
                    .map(|l| l.id)
                    .collect();
                if !outcome.malformed.is_empty() {
                    tracing::warn!(
                        "play log: {} listen(s) refused as malformed, dropped",
                        outcome.malformed.len()
                    );
                }
                tracing::debug!("play log: {} listen(s) accepted", outcome.accepted.len());
                let complete = done.len() == pending.len();
                drop_logs(&store, done).await;
                if complete {
                    batch = MAX_BATCH;
                    backoff = MIN_BACKOFF;
                } else {
                    tracing::warn!("play log: some listens not taken, retry in {backoff:?}");
                    wait(backoff).await;
                    backoff = (backoff * 2).min(MAX_BACKOFF);
                }
            }
            Err(TidalError::NotLoggedIn) => wait(SESSION_POLL).await,
            Err(TidalError::Api { status, .. })
                if (400..500).contains(&status) && status != 401 && status != 429 =>
            {
                if pending.len() > 1 {
                    batch = 1;
                } else {
                    tracing::warn!("play log: listen refused ({status}), dropped");
                    drop_logs(&store, pending.iter().map(|l| l.id).collect()).await;
                }
            }
            Err(e) => {
                tracing::warn!("play log: not sent ({e}), retry in {backoff:?}");
                wait(backoff).await;
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
        }
    }
}

/// Hooks of the player: each one feeds `PlayLogTracker` and queues what it ends. Nothing
/// during a blind test (#434): its clips are not listens.
impl Player {
    /// Stream started at entry 0 from `start` s; `album`: the served track's album.
    pub(super) fn log_loaded(&mut self, session: PlaybackSession, album: u64, start: f64) {
        let ended = if self.blind.is_some() {
            self.playlog.close(now_ms())
        } else {
            let source = self.log_source(album);
            self.playlog.loaded(session, source, start, now_ms())
        };
        self.send_logs(ended);
    }

    /// Gapless move to the preloaded stream.
    pub(super) fn log_advanced(&mut self, session: PlaybackSession, album: u64, previous: f64) {
        let source = self.log_source(album);
        let ended = self.playlog.advanced(session, source, previous, now_ms());
        self.send_logs(ended);
    }

    pub(super) fn log_position(&mut self) {
        if self.blind.is_some() {
            return;
        }
        let s = &self.state;
        let ended = self.playlog.position(
            s.time_pos,
            s.duration,
            s.paused,
            now_ms(),
            playlog::new_uuid,
        );
        self.send_logs(ended);
    }

    pub(super) fn log_paused(&mut self, paused: bool) {
        if self.blind.is_some() {
            return;
        }
        self.playlog.paused(paused, now_ms());
    }

    pub(super) fn log_seek(&mut self, to: f64) {
        if self.blind.is_some() {
            return;
        }
        self.playlog.seek(to, now_ms());
    }

    /// Natural end of the current track, nothing preloaded.
    pub(super) fn log_ended(&mut self) {
        let ended = self.playlog.ended(self.state.duration, now_ms());
        self.send_logs(ended);
    }

    /// Playback of the current stream stops here.
    pub(super) fn log_close(&mut self) {
        let ended = self.playlog.close(now_ms());
        self.send_logs(ended);
    }

    /// Ends the current listen for the closing save (#260), to be awaited with it.
    pub(super) fn log_close_for_exit(&mut self) -> Option<impl Future<Output = ()> + use<>> {
        let ended = self.playlog.close(now_ms())?;
        let (tidal, store) = (Arc::clone(&self.tidal), self.store.clone()?);
        Some(async move { queue(&tidal, &store, vec![ended]).await })
    }

    fn send_logs(&self, ended: Option<PlayLog>) {
        let (Some(log), Some(store)) = (ended, self.store.clone()) else {
            return;
        };
        let tidal = Arc::clone(&self.tidal);
        tauri::async_runtime::spawn(async move { queue(&tidal, &store, vec![log]).await });
    }

    /// Source of the context, as the play log names it; the album otherwise (liked
    /// tracks, search, radio…).
    fn log_source(&self, album: u64) -> LogSource {
        if self.queue.radio_playing().is_some() {
            return LogSource::Album(album);
        }
        match self.queue.source() {
            Some(PlaySource::Album { id, .. }) => LogSource::Album(*id),
            Some(PlaySource::Playlist { uuid, .. }) => LogSource::Playlist(uuid.clone()),
            Some(PlaySource::Mix { id, .. }) => LogSource::Mix(id.clone()),
            Some(PlaySource::Artist { id, .. }) => LogSource::Artist(*id),
            _ => LogSource::Album(album),
        }
    }
}

/// Waits `delay`, or less if a listen is queued meanwhile.
async fn wait(delay: Duration) {
    tokio::select! {
        () = WAKE.notified() => {}
        () = tokio::time::sleep(delay) => {}
    }
}

async fn drop_logs(store: &Arc<Store>, ids: Vec<i64>) {
    let store = Arc::clone(store);
    match tokio::task::spawn_blocking(move || store.drop_play_logs(&ids)).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::warn!("play log: sent listens not removed: {e}"),
        Err(e) => tracing::warn!("play log: sent listens not removed: {e}"),
    }
}
