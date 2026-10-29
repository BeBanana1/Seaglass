//! Acteur lecteur : seule tâche qui possède `PlayerState` et le handle mpv.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter};
use tokio::process::Child;
use tokio::sync::{mpsc, oneshot};

use super::state::PlayerState;
use crate::mpv::{self, EndReason, MpvEvent, MpvHandle, Property, serve::ManifestServer};
use crate::tidal::TidalClient;
use crate::tidal::models::{AudioQuality, Stream, StreamSource, Track, TrackId};

/// Intervalle minimal entre deux événements qui ne changent que `time_pos` (~4 Hz).
const TIME_EMIT_INTERVAL: Duration = Duration::from_millis(250);
/// Qualité demandée ; deviendra un réglage utilisateur.
const QUALITY: AudioQuality = AudioQuality::HiResLossless;

pub enum Cmd {
    Play(TrackId),
    TogglePause,
    Seek(f64),
    GetState(oneshot::Sender<PlayerState>),
    /// Résultat de la résolution réseau lancée par `Play`.
    Resolved {
        request: u64,
        result: Result<Box<(Track, Stream)>, String>,
    },
}

#[derive(Clone, Serialize)]
struct Toast {
    level: &'static str,
    message: String,
}

struct Mpv {
    handle: MpvHandle,
    _child: Child,
}

struct Player {
    app: AppHandle,
    tidal: Arc<TidalClient>,
    tx: mpsc::Sender<Cmd>,
    mpv: Result<Mpv, String>,
    server: Option<ManifestServer>,
    state: PlayerState,
    emitted: PlayerState,
    last_emit: Instant,
    request: u64,
}

pub fn spawn(app: AppHandle, tidal: Arc<TidalClient>, mpv_conf: PathBuf) -> mpsc::Sender<Cmd> {
    let (tx, rx) = mpsc::channel(64);
    let actor_tx = tx.clone();
    tauri::async_runtime::spawn(async move {
        let (events_tx, events_rx) = mpsc::channel(256);
        let mpv = start_mpv(&mpv_conf, events_tx).await;
        if let Err(e) = &mpv {
            tracing::error!("{e}");
        }
        let server = match ManifestServer::start().await {
            Ok(s) => Some(s),
            Err(e) => {
                tracing::error!("{e}");
                None
            }
        };
        let player = Player {
            app,
            tidal,
            tx: actor_tx,
            mpv,
            server,
            state: PlayerState {
                idle: true,
                ..PlayerState::default()
            },
            emitted: PlayerState::default(),
            last_emit: Instant::now(),
            request: 0,
        };
        player.run(rx, events_rx).await;
    });
    tx
}

async fn start_mpv(conf: &std::path::Path, events: mpsc::Sender<MpvEvent>) -> Result<Mpv, String> {
    let child = mpv::process::spawn(conf).map_err(|e| e.to_string())?;
    let handle = mpv::ipc::connect(events).await.map_err(|e| e.to_string())?;
    Ok(Mpv {
        handle,
        _child: child,
    })
}

impl Player {
    async fn run(mut self, mut cmds: mpsc::Receiver<Cmd>, mut events: mpsc::Receiver<MpvEvent>) {
        loop {
            tokio::select! {
                cmd = cmds.recv() => match cmd {
                    Some(cmd) => self.handle_cmd(cmd).await,
                    None => break,
                },
                Some(ev) = events.recv() => self.handle_event(ev),
            }
            self.emit_state();
        }
    }

    async fn handle_cmd(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Play(id) => self.play(id),
            Cmd::Resolved { request, result } if request == self.request => {
                self.state.loading = false;
                match result {
                    Ok(resolved) => {
                        let (track, stream) = *resolved;
                        self.load(track, stream).await
                    }
                    Err(e) => self.toast("error", e),
                }
            }
            Cmd::Resolved { .. } => {} // Remplacé par un `Play` plus récent.
            Cmd::TogglePause => self.mpv_command(vec![json!("cycle"), json!("pause")]).await,
            Cmd::Seek(secs) => {
                self.mpv_command(vec![json!("seek"), json!(secs), json!("absolute")])
                    .await
            }
            Cmd::GetState(reply) => {
                let _ = reply.send(self.state.clone());
            }
        }
    }

    /// Résout métadonnées et flux hors de l'acteur, qui reste réactif pendant ce temps.
    fn play(&mut self, id: TrackId) {
        if let Err(e) = &self.mpv {
            let message = e.clone();
            self.toast("error", message);
            return;
        }
        self.request += 1;
        self.state.loading = true;
        let (request, tidal, tx) = (self.request, Arc::clone(&self.tidal), self.tx.clone());
        tauri::async_runtime::spawn(async move {
            let result = async {
                let track = tidal.track(id).await?;
                let stream = tidal.stream(id, QUALITY).await?;
                Ok::<_, crate::tidal::TidalError>(Box::new((track, stream)))
            }
            .await
            .map_err(|e| e.to_string());
            let _ = tx.send(Cmd::Resolved { request, result }).await;
        });
    }

    async fn load(&mut self, track: Track, stream: Stream) {
        let url = match (&stream.source, &self.server) {
            (StreamSource::Direct(url), _) => url.clone(),
            (StreamSource::Dash(mpd), Some(server)) => {
                server.publish(&track.id.to_string(), mpd.clone())
            }
            (StreamSource::Dash(_), None) => {
                self.toast("error", "serveur de manifestes indisponible".into());
                return;
            }
        };
        tracing::info!(
            "lecture {} : {:?} {} {:?}/{:?}",
            track.id,
            stream.format.quality,
            stream.format.codec,
            stream.format.bit_depth,
            stream.format.sample_rate
        );
        self.state.track = Some(track);
        self.state.stream = Some(stream.format);
        self.state.time_pos = 0.0;
        self.mpv_command(vec![json!("loadfile"), json!(url), json!("replace")])
            .await;
        // Lancer un titre reprend la lecture si mpv était en pause.
        if let Ok(mpv) = &self.mpv
            && let Err(e) = mpv.handle.set_property("pause", json!(false)).await
        {
            tracing::warn!("reprise impossible : {e}");
        }
    }

    fn handle_event(&mut self, ev: MpvEvent) {
        let s = &mut self.state;
        match ev {
            MpvEvent::Property(p) => match p {
                Property::TimePos(t) => s.time_pos = t.unwrap_or(0.0),
                Property::Duration(d) => s.duration = d.unwrap_or(0.0),
                Property::Pause(p) => s.paused = p,
                Property::PausedForCache(b) => s.buffering = b,
                Property::AudioParams(a) => s.output = a,
                Property::CurrentAo(ao) => s.output_driver = ao,
                Property::IdleActive(i) => s.idle = i,
                Property::PlaylistPos(_) => {}
            },
            MpvEvent::EndFile {
                reason: EndReason::Error,
                error,
            } => {
                let detail = error.unwrap_or_else(|| "erreur inconnue".into());
                self.toast("error", format!("lecture interrompue : {detail}"));
            }
            MpvEvent::Disconnected => {
                self.mpv = Err("mpv s'est arrêté, redémarrer l'app".into());
                self.toast("error", "mpv s'est arrêté".into());
            }
            MpvEvent::EndFile { .. } | MpvEvent::StartFile | MpvEvent::FileLoaded => {}
        }
    }

    async fn mpv_command(&mut self, command: Vec<serde_json::Value>) {
        let result = match &self.mpv {
            Ok(mpv) => mpv
                .handle
                .command(command)
                .await
                .map(drop)
                .map_err(|e| e.to_string()),
            Err(e) => Err(e.clone()),
        };
        if let Err(e) = result {
            self.toast("error", e);
        }
    }

    fn emit_state(&mut self) {
        if self.state == self.emitted {
            return;
        }
        if self.emitted.differs_only_by_time(&self.state)
            && self.last_emit.elapsed() < TIME_EMIT_INTERVAL
        {
            return;
        }
        if let Err(e) = self.app.emit("player-state", &self.state) {
            tracing::warn!("émission player-state : {e}");
        }
        self.emitted = self.state.clone();
        self.last_emit = Instant::now();
    }

    fn toast(&self, level: &'static str, message: String) {
        tracing::warn!("{message}");
        let _ = self.app.emit("toast", Toast { level, message });
    }
}
