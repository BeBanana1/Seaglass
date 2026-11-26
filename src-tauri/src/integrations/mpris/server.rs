//! MPRIS server on the session bus (`zbus` 5, already compiled on Linux by the `opener` and
//! `single-instance` plugins). The actor notifies `NowPlaying` through a `watch`; a task updates
//! the `Player` interface and only emits `PropertiesChanged` for what changes.

use std::collections::HashMap;
use std::time::Instant;

use tauri::AppHandle;
use tokio::sync::{mpsc, watch};
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{ObjectPath, OwnedValue, Value};
use zbus::{connection, fdo, interface};

use super::logic::{self, Jump, State};
use crate::integrations::NowPlaying;
use crate::integrations::tray;
use crate::player::Cmd;

const BUS_NAME: &str = "org.mpris.MediaPlayer2.seaglass";
const OBJECT_PATH: &str = "/org/mpris/MediaPlayer2";

pub struct MprisHandle {
    tx: watch::Sender<Option<NowPlaying>>,
}

impl MprisHandle {
    pub fn update(&self, state: NowPlaying) {
        self.tx.send_replace(Some(state));
    }
}

/// Starts the server in the background; a missing session bus is only logged.
pub fn start(app: AppHandle, player: mpsc::Sender<Cmd>) -> MprisHandle {
    let (tx, rx) = watch::channel(None);
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run(app, player, rx).await {
            tracing::warn!("MPRIS unavailable: {e}");
        }
    });
    MprisHandle { tx }
}

async fn run(
    app: AppHandle,
    player: mpsc::Sender<Cmd>,
    mut rx: watch::Receiver<Option<NowPlaying>>,
) -> zbus::Result<()> {
    let conn = connection::Builder::session()?
        .name(BUS_NAME)?
        .serve_at(OBJECT_PATH, Root { app })?
        .serve_at(
            OBJECT_PATH,
            Player {
                player,
                state: State::default(),
            },
        )?
        .build()
        .await?;
    tracing::info!("MPRIS: {BUS_NAME} published");
    let iface = conn
        .object_server()
        .interface::<_, Player>(OBJECT_PATH)
        .await?;
    while rx.changed().await.is_ok() {
        let Some(np) = rx.borrow_and_update().clone() else {
            continue;
        };
        let emitter = iface.signal_emitter();
        let mut player = iface.get_mut().await;
        let changes = player.state.update(np, Instant::now());
        if let Err(e) = player.emit(&changes, emitter).await {
            tracing::warn!("MPRIS: {e}");
        }
    }
    Ok(())
}

/// `org.mpris.MediaPlayer2`: the app's identity and showing the window again.
struct Root {
    app: AppHandle,
}

#[interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    fn raise(&self) {
        tray::show_main(&self.app);
    }

    /// `CanQuit` is false: quitting stays a gesture made in the app or the notification area.
    fn quit(&self) {}

    #[zbus(property(emits_changed_signal = "const"))]
    fn can_quit(&self) -> bool {
        false
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn can_raise(&self) -> bool {
        true
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn has_track_list(&self) -> bool {
        false
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn identity(&self) -> &str {
        "Seaglass"
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn supported_uri_schemes(&self) -> Vec<String> {
        Vec::new()
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn supported_mime_types(&self) -> Vec<String> {
        Vec::new()
    }
}

/// `org.mpris.MediaPlayer2.Player`: playback state and commands to the actor.
struct Player {
    player: mpsc::Sender<Cmd>,
    state: State,
}

impl Player {
    async fn jump(&self, jump: Option<Jump>) {
        match jump {
            Some(Jump::To(position)) => self.send(Cmd::Seek(position)).await,
            Some(Jump::Next) => self.send(Cmd::Next).await,
            None => {}
        }
    }

    async fn send(&self, cmd: Cmd) {
        if self.player.send(cmd).await.is_err() {
            tracing::warn!("MPRIS: player unavailable");
        }
    }

    async fn emit(&self, c: &logic::Changes, e: &SignalEmitter<'_>) -> zbus::Result<()> {
        if c.metadata {
            self.metadata_changed(e).await?;
        }
        if c.status {
            self.playback_status_changed(e).await?;
        }
        if c.loop_status {
            self.loop_status_changed(e).await?;
        }
        if c.shuffle {
            self.shuffle_changed(e).await?;
        }
        if c.volume {
            self.volume_changed(e).await?;
        }
        if c.controls {
            self.can_go_next_changed(e).await?;
            self.can_go_previous_changed(e).await?;
            self.can_play_changed(e).await?;
            self.can_pause_changed(e).await?;
            self.can_seek_changed(e).await?;
        }
        if let Some(position) = c.seeked {
            Self::seeked(e, logic::micros(position)).await?;
        }
        Ok(())
    }
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    async fn next(&self) {
        self.send(Cmd::Next).await;
    }

    async fn previous(&self) {
        self.send(Cmd::Previous).await;
    }

    async fn pause(&self) {
        self.send(Cmd::SetPaused(true)).await;
    }

    async fn play_pause(&self) {
        self.send(Cmd::TogglePause).await;
    }

    /// The player has no stop: pausing stands in for it.
    async fn stop(&self) {
        self.send(Cmd::SetPaused(true)).await;
    }

    async fn play(&self) {
        self.send(Cmd::SetPaused(false)).await;
    }

    async fn seek(&self, offset: i64) {
        self.jump(self.state.seek(offset, Instant::now())).await;
    }

    async fn set_position(&self, track_id: ObjectPath<'_>, position: i64) {
        self.jump(self.state.set_position(track_id.as_str(), position))
            .await;
    }

    fn open_uri(&self, _uri: &str) -> fdo::Result<()> {
        Err(fdo::Error::NotSupported("OpenUri".into()))
    }

    #[zbus(signal)]
    async fn seeked(emitter: &SignalEmitter<'_>, position: i64) -> zbus::Result<()>;

    #[zbus(property)]
    fn playback_status(&self) -> &str {
        self.state.status()
    }

    #[zbus(property)]
    fn loop_status(&self) -> &str {
        self.state
            .now_playing()
            .map_or("None", |np| logic::loop_status(np.repeat))
    }

    #[zbus(property)]
    async fn set_loop_status(&mut self, value: String) -> fdo::Result<()> {
        let repeat = logic::parse_loop_status(&value)
            .ok_or_else(|| fdo::Error::InvalidArgs(format!("LoopStatus inconnu : {value}")))?;
        self.send(Cmd::SetRepeat(repeat)).await;
        Ok(())
    }

    #[zbus(property)]
    fn rate(&self) -> f64 {
        1.0
    }

    /// A single rate: the specification asks to ignore any other value.
    #[zbus(property)]
    fn set_rate(&mut self, _rate: f64) {}

    #[zbus(property)]
    fn shuffle(&self) -> bool {
        self.state.now_playing().is_some_and(|np| np.shuffle)
    }

    #[zbus(property)]
    async fn set_shuffle(&mut self, value: bool) {
        self.send(Cmd::SetShuffle(value)).await;
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedValue> {
        let mut map = HashMap::new();
        let Some(meta) = self.state.metadata() else {
            if let Ok(path) = ObjectPath::try_from(logic::NO_TRACK) {
                insert(&mut map, "mpris:trackid", path);
            }
            return map;
        };
        if let Ok(path) = ObjectPath::try_from(meta.track_id) {
            insert(&mut map, "mpris:trackid", path);
        }
        insert(&mut map, "mpris:length", meta.length);
        insert(&mut map, "xesam:title", meta.title);
        insert(&mut map, "xesam:artist", meta.artists);
        insert(&mut map, "xesam:album", meta.album);
        if let Some(url) = meta.art_url {
            insert(&mut map, "mpris:artUrl", url);
        }
        map
    }

    #[zbus(property)]
    fn volume(&self) -> f64 {
        self.state.now_playing().map_or(1.0, |np| np.volume)
    }

    #[zbus(property)]
    async fn set_volume(&mut self, value: f64) {
        self.send(Cmd::SetVolume(logic::volume_level(value))).await;
    }

    /// Read on demand and never signaled (specification): `Seeked` covers the jumps.
    #[zbus(property(emits_changed_signal = "false"))]
    fn position(&self) -> i64 {
        logic::micros(self.state.position(Instant::now()))
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn minimum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn maximum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        self.state.can_go_next()
    }

    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        self.state.can_go_previous()
    }

    #[zbus(property)]
    fn can_play(&self) -> bool {
        self.state.can_play()
    }

    #[zbus(property)]
    fn can_pause(&self) -> bool {
        self.state.can_play()
    }

    #[zbus(property)]
    fn can_seek(&self) -> bool {
        self.state.can_seek()
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn can_control(&self) -> bool {
        true
    }
}

fn insert<'a>(map: &mut HashMap<String, OwnedValue>, key: &str, value: impl Into<Value<'a>>) {
    if let Ok(value) = OwnedValue::try_from(value.into()) {
        map.insert(key.to_owned(), value);
    }
}
