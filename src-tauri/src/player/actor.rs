//! Acteur lecteur : seule tâche qui possède la file, `PlayerState` et le handle mpv.
//! La gestion de la fenêtre mpv (gapless, erreurs) est dans `playback.rs`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot};

use super::queue::{Queue, Repeat};
use super::state::{PlayerState, QueueView};
use crate::mpv::process::ProcessGuard;
use crate::mpv::{self, MpvEvent, MpvHandle, Property, serve::ManifestServer};
use crate::store::Store;
use crate::tidal::TidalClient;
use crate::tidal::models::{Stream, StreamFormat, Track, TrackId};

/// Intervalle minimal entre deux événements qui ne changent que `time_pos` (~4 Hz).
const TIME_EMIT_INTERVAL: Duration = Duration::from_millis(250);
/// Nombre de titres du contexte envoyés à l'UI dans `queue-state`.
const UPCOMING_LIMIT: usize = 50;

pub enum Cmd {
    PlayTracks {
        ids: Vec<TrackId>,
        start: usize,
    },
    PlayNext(TrackId),
    AddToQueue(TrackId),
    RemoveFromQueue(usize),
    Next,
    Previous,
    TogglePause,
    Seek(f64),
    SetShuffle(bool),
    SetRepeat(Repeat),
    GetState(oneshot::Sender<PlayerState>),
    GetQueue(oneshot::Sender<QueueView>),
    /// Résultat d'une résolution réseau (métadonnées + flux) lancée par l'acteur.
    Resolved {
        request: u64,
        slot: Slot,
        result: Result<Box<(Track, Stream)>, String>,
    },
}

/// Destination d'un flux résolu dans la fenêtre mpv.
#[derive(Debug, Clone, Copy)]
pub enum Slot {
    /// Entrée 0, chargée en `replace` à partir de `start` secondes.
    Current { start: f64 },
    /// Entrée 1, préchargée en `append` pour le gapless.
    Next(TrackId),
}

#[derive(Clone, Serialize)]
struct Toast {
    level: &'static str,
    message: String,
}

pub(super) struct Mpv {
    pub(super) handle: MpvHandle,
}

pub(super) struct Player {
    pub(super) app: AppHandle,
    pub(super) tidal: Arc<TidalClient>,
    pub(super) store: Option<Arc<Store>>,
    pub(super) tx: mpsc::Sender<Cmd>,
    pub(super) mpv: Result<Mpv, String>,
    pub(super) server: Option<ManifestServer>,
    pub(super) rng: StdRng,
    pub(super) queue: Queue,
    pub(super) state: PlayerState,
    emitted: PlayerState,
    last_emit: Instant,
    /// Compteurs qui invalident les résolutions devenues obsolètes.
    pub(super) load_req: u64,
    pub(super) preload_req: u64,
    /// mpv a le titre courant en entrée 0 (faux après restauration, avant lecture).
    pub(super) loaded: bool,
    pub(super) preloading: bool,
    /// Titre préchargé en entrée 1 de mpv.
    pub(super) next_loaded: Option<(Track, StreamFormat)>,
    /// Titre pour lequel une nouvelle URL a déjà été demandée après une erreur.
    pub(super) retried: Option<TrackId>,
    pub(super) failures: u32,
    pub(super) queue_dirty: bool,
    pub(super) persist_queue: bool,
    pub(super) persist_position: bool,
    pub(super) last_persist: Instant,
}

pub fn spawn(
    app: AppHandle,
    tidal: Arc<TidalClient>,
    store: Option<Arc<Store>>,
    mpv_conf: PathBuf,
    mpv_process: Arc<ProcessGuard>,
) -> mpsc::Sender<Cmd> {
    let (tx, rx) = mpsc::channel(64);
    let actor_tx = tx.clone();
    tauri::async_runtime::spawn(async move {
        let (events_tx, events_rx) = mpsc::channel(256);
        let mpv = start_mpv(&mpv_conf, events_tx, &mpv_process).await;
        if let Err(e) = &mpv {
            tracing::error!("{e}");
        }
        let server = ManifestServer::start()
            .await
            .inspect_err(|e| tracing::error!("{e}"))
            .ok();
        let mut player = Player {
            app,
            tidal,
            store,
            tx: actor_tx,
            mpv,
            server,
            rng: StdRng::from_os_rng(),
            queue: Queue::default(),
            state: PlayerState {
                idle: true,
                ..PlayerState::default()
            },
            emitted: PlayerState::default(),
            last_emit: Instant::now(),
            load_req: 0,
            preload_req: 0,
            loaded: false,
            preloading: false,
            next_loaded: None,
            retried: None,
            failures: 0,
            queue_dirty: true,
            persist_queue: false,
            persist_position: false,
            last_persist: Instant::now(),
        };
        player.restore();
        player.apply_loop_file().await;
        player.run(rx, events_rx).await;
    });
    tx
}

async fn start_mpv(
    conf: &std::path::Path,
    events: mpsc::Sender<MpvEvent>,
    process: &ProcessGuard,
) -> Result<Mpv, String> {
    process.set(mpv::process::spawn(conf).map_err(|e| e.to_string())?);
    let handle = mpv::ipc::connect(events).await.map_err(|e| e.to_string())?;
    Ok(Mpv { handle })
}

impl Player {
    async fn run(mut self, mut cmds: mpsc::Receiver<Cmd>, mut events: mpsc::Receiver<MpvEvent>) {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                cmd = cmds.recv() => match cmd {
                    Some(cmd) => self.handle_cmd(cmd).await,
                    None => break,
                },
                Some(ev) = events.recv() => self.handle_event(ev).await,
                _ = tick.tick() => self.persist_if_due(),
            }
            self.emit();
        }
    }

    async fn handle_cmd(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::PlayTracks { ids, start } => {
                let started = self.queue.set_context(ids, start, &mut self.rng);
                if let Some(id) = started {
                    self.on_queue_changed().await;
                    self.load_current(id, 0.0);
                }
            }
            Cmd::PlayNext(id) => {
                self.queue.play_next(id);
                self.on_queue_changed().await;
            }
            Cmd::AddToQueue(id) => {
                self.queue.add_to_queue(id);
                self.on_queue_changed().await;
            }
            Cmd::RemoveFromQueue(index) => {
                if self.queue.remove_manual(index) {
                    self.on_queue_changed().await;
                }
            }
            Cmd::Next => self.skip_next().await,
            Cmd::Previous => self.previous().await,
            Cmd::TogglePause => self.toggle_pause().await,
            Cmd::Seek(secs) => self.seek(secs).await,
            Cmd::SetShuffle(on) => {
                self.queue.set_shuffle(on, &mut self.rng);
                self.on_queue_changed().await;
            }
            Cmd::SetRepeat(repeat) => {
                self.queue.set_repeat(repeat);
                self.apply_loop_file().await;
                self.on_queue_changed().await;
            }
            Cmd::GetState(reply) => {
                let _ = reply.send(self.state.clone());
            }
            Cmd::GetQueue(reply) => {
                let _ = reply.send(self.queue_view());
            }
            Cmd::Resolved {
                request,
                slot,
                result,
            } => self.on_resolved(request, slot, result).await,
        }
    }

    async fn handle_event(&mut self, ev: MpvEvent) {
        let s = &mut self.state;
        match ev {
            // Position et durée ne valent que pour un titre chargé : avant, elles écraseraient
            // la position restaurée ou celle du titre en cours de résolution.
            MpvEvent::Property(p) => match p {
                Property::TimePos(t) if self.loaded => {
                    s.time_pos = t.unwrap_or(0.0);
                    self.persist_position = true;
                    self.maybe_preload();
                }
                Property::Duration(d) if self.loaded => s.duration = d.unwrap_or(0.0),
                Property::TimePos(_) | Property::Duration(_) => {}
                Property::Pause(p) => {
                    s.paused = p;
                    self.persist_position = true;
                }
                Property::PausedForCache(b) => s.buffering = b,
                Property::AudioParams(a) => s.output = a,
                Property::CurrentAo(ao) => s.output_driver = ao,
                Property::IdleActive(i) => s.idle = i,
                Property::PlaylistPos(pos) => self.on_playlist_pos(pos).await,
            },
            MpvEvent::FileLoaded => self.failures = 0,
            MpvEvent::EndFile { reason, error } => self.on_end_file(reason, error).await,
            MpvEvent::Disconnected => {
                self.mpv = Err("mpv s'est arrêté, redémarrer l'app".into());
                self.loaded = false;
                self.next_loaded = None;
                self.toast("error", "mpv s'est arrêté".into());
            }
            MpvEvent::StartFile => {}
        }
    }

    /// Toute modification de la file invalide le titre préchargé (fenêtre mpv de 2 entrées).
    pub(super) async fn on_queue_changed(&mut self) {
        self.invalidate_next().await;
        self.mark_queue_dirty();
    }

    pub(super) fn mark_queue_dirty(&mut self) {
        self.queue_dirty = true;
        self.persist_queue = true;
    }

    fn queue_view(&self) -> QueueView {
        QueueView {
            manual: self.tidal.cache.get_many(self.queue.manual()),
            upcoming: self
                .tidal
                .cache
                .get_many(self.queue.upcoming_context(UPCOMING_LIMIT)),
        }
    }

    fn emit(&mut self) {
        let s = &mut self.state;
        s.shuffle = self.queue.shuffle();
        s.repeat = self.queue.repeat();
        s.has_previous = self.queue.has_previous();
        s.has_next = self.queue.peek_next().is_some();

        if self.queue_dirty {
            self.queue_dirty = false;
            if let Err(e) = self.app.emit("queue-state", self.queue_view()) {
                tracing::warn!("émission queue-state : {e}");
            }
        }
        if self.state == self.emitted
            || (self.emitted.differs_only_by_time(&self.state)
                && self.last_emit.elapsed() < TIME_EMIT_INTERVAL)
        {
            return;
        }
        if let Err(e) = self.app.emit("player-state", &self.state) {
            tracing::warn!("émission player-state : {e}");
        }
        self.emitted = self.state.clone();
        self.last_emit = Instant::now();
    }

    pub(super) fn toast(&self, level: &'static str, message: String) {
        tracing::warn!("{message}");
        let _ = self.app.emit("toast", Toast { level, message });
    }
}
