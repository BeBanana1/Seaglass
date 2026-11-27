//! mpv engine: translates `engine::Op` operations into JSON IPC commands and mpv's events
//! into `engine::Event`. Pure tested logic, except `MpvEngine::start`.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::device::INIT_FAILED;
use super::events::{EndReason, MpvEvent, Property};
use super::filters::{af_value, file_options};
use super::process::ProcessGuard;
use super::{CacheSize, MpvHandle};
use crate::engine::{End, Engine, Event, Op};
use crate::i18n::tr;

/// Time mpv has to answer a command. Even `set_property audio-device`, which reopens the
/// device, answers well before; past it, mpv is considered frozen (#255).
const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

pub struct MpvEngine {
    handle: MpvHandle,
    process: Arc<ProcessGuard>,
}

/// What it takes to start mpv: its dedicated config and the process guard (stopped when
/// the app exits and before an update).
pub struct Launch {
    pub conf: std::path::PathBuf,
    pub process: Arc<ProcessGuard>,
}

impl Launch {
    pub async fn start(
        &self,
        cache: CacheSize,
        audio_device: Option<&str>,
        exclusive: bool,
        events: mpsc::Sender<Event>,
    ) -> Result<MpvEngine, String> {
        let output = Output {
            device: audio_device,
            exclusive,
        };
        MpvEngine::start(&self.conf, cache, output, &self.process, events).await
    }

    /// False while the app closes or updates: mpv was stopped on purpose.
    pub fn may_restart(&self) -> bool {
        !self.process.closing()
    }
}

/// Output asked for at launch: device (`None` = system default) and
/// exclusive mode (#245).
#[derive(Debug, Clone, Copy, Default)]
pub struct Output<'a> {
    pub device: Option<&'a str>,
    pub exclusive: bool,
}

impl MpvEngine {
    /// Starts mpv (process kept by `process`), connects to it and forwards its translated
    /// events to `events`.
    pub async fn start(
        conf: &std::path::Path,
        cache: CacheSize,
        output: Output<'_>,
        process: &Arc<ProcessGuard>,
        events: mpsc::Sender<Event>,
    ) -> Result<Self, String> {
        let child = super::process::spawn(conf, cache, output.device, output.exclusive)
            .map_err(|e| e.to_string())?;
        process.set(child);
        let (raw_tx, mut raw_rx) = mpsc::channel(256);
        let handle = super::ipc::connect(raw_tx)
            .await
            .map_err(|e| e.to_string())?;
        tokio::spawn(async move {
            while let Some(ev) = raw_rx.recv().await {
                if let Some(ev) = translate(ev)
                    && events.send(ev).await.is_err()
                {
                    break;
                }
            }
        });
        Ok(Self {
            handle,
            process: Arc::clone(process),
        })
    }

    /// Sends the operation's commands in order, up to the first refused one. With no reply
    /// within `limit`, mpv is frozen: it is stopped, and the pipe closing gives
    /// `Event::Stopped`, so the actor restarts it (#255). Without this timeout, the actor
    /// would stay stuck on the command and nothing would answer anymore.
    async fn apply_within(&self, op: Op, limit: Duration) -> Result<(), String> {
        for command in commands(&op) {
            match tokio::time::timeout(limit, self.handle.command(command)).await {
                Ok(reply) => reply.map_err(|e| e.to_string())?,
                Err(_) => {
                    tracing::error!("mpv has not answered for {limit:?}, stopping the process");
                    self.process.terminate();
                    return Err(tr("mpv ne répond plus", "mpv is not responding").into());
                }
            };
        }
        Ok(())
    }
}

impl Engine for MpvEngine {
    async fn apply(&self, op: Op) -> Result<(), String> {
        self.apply_within(op, COMMAND_TIMEOUT).await
    }
}

/// mpv commands of an operation.
pub fn commands(op: &Op) -> Vec<Value> {
    let set = |name: &str, value: Value| json!(["set_property", name, value]);
    // Named arguments: positionally, mpv 0.38 inserted `index` before `options`, and
    // Ubuntu 24.04's mpv 0.37 refuses `… replace -1 start=…` (#316). Both understand them.
    let load = |url: &str, mode: &str, options: Option<String>| {
        let mut cmd = json!({ "name": "loadfile", "url": url, "flags": mode });
        if let Some(options) = options {
            cmd["options"] = json!(options);
        }
        cmd
    };
    match op {
        Op::Play {
            url,
            start,
            filters,
        } => vec![
            load(url, "replace", file_options(*start, filters)),
            set("pause", json!(false)),
        ],
        Op::Preload { url, filters } => vec![load(url, "append", file_options(0.0, filters))],
        Op::RemoveNext => vec![json!(["playlist-remove", 1])],
        Op::DropPrevious => vec![json!(["playlist-remove", 0])],
        Op::Stop => vec![json!(["stop"])],
        Op::TogglePause => vec![json!(["cycle", "pause"])],
        Op::SetPaused(paused) => vec![set("pause", json!(paused))],
        Op::Seek(secs) => vec![json!(["seek", secs, "absolute"])],
        Op::LoopCurrent(on) => vec![set("loop-file", json!(if *on { "inf" } else { "no" }))],
        Op::Volume { level, muted } => {
            vec![set("volume", json!(level)), set("mute", json!(muted))]
        }
        Op::CacheSize(size) => size
            .set_property_commands()
            .into_iter()
            .map(Value::from)
            .collect(),
        // File-local option: reset at the end of the track, the preloaded one keeps its own.
        Op::CurrentFilters(filters) => {
            vec![set("file-local-options/af", json!(af_value(filters)))]
        }
        Op::OutputDevice(id) => vec![set("audio-device", json!(id))],
        Op::Exclusive(on) => vec![set("audio-exclusive", json!(on))],
    }
}

/// mpv event as the actor sees it; `None` if the actor does not care about it.
pub fn translate(ev: MpvEvent) -> Option<Event> {
    Some(match ev {
        MpvEvent::Property(p) => match p {
            // Window of 2 entries: moving to entry 1 means moving on to the preloaded track.
            Property::PlaylistPos(Some(1)) => Event::Advanced,
            Property::PlaylistPos(_) => return None,
            Property::TimePos(t) => Event::Position(t),
            Property::Duration(d) => Event::Duration(d),
            Property::Pause(p) => Event::Paused(p),
            Property::PausedForCache(b) => Event::Buffering(b),
            Property::AudioParams(a) => Event::Format(a),
            Property::CurrentAo(ao) => Event::Driver(ao),
            Property::IdleActive(i) => Event::Idle(i),
            Property::AudioDeviceList(list) => Event::Devices(list),
            Property::AudioOutParams(a) => Event::DeviceFormat(a),
            Property::DemuxerCacheTime(t) => Event::Cached(t),
        },
        MpvEvent::FileLoaded => Event::Loaded,
        MpvEvent::EndFile { reason, error } => Event::Ended(match reason {
            EndReason::Eof => End::Eof,
            EndReason::Error if error.as_deref() == Some(INIT_FAILED) => End::OutputFailed,
            EndReason::Error => End::Error(error),
            EndReason::Stop | EndReason::Quit | EndReason::Redirect | EndReason::Unknown => {
                return None;
            }
        }),
        MpvEvent::Disconnected => Event::Stopped,
        MpvEvent::StartFile => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Filters;

    fn gain(g: Option<f64>) -> Filters {
        Filters { gain: g, eq: None }
    }

    #[test]
    fn play_replaces_and_unpauses() {
        let play = Op::Play {
            url: "http://x/a.mpd".into(),
            start: 0.0,
            filters: gain(None),
        };
        assert_eq!(
            commands(&play),
            vec![
                json!({ "name": "loadfile", "url": "http://x/a.mpd", "flags": "replace" }),
                json!(["set_property", "pause", false]),
            ]
        );
        let resumed = Op::Play {
            url: "u".into(),
            start: 42.0,
            filters: gain(Some(-6.5)),
        };
        assert_eq!(
            commands(&resumed)[0],
            json!({
                "name": "loadfile",
                "url": "u",
                "flags": "replace",
                "options": "start=42,af=volume=volume=-6.50dB"
            })
        );
    }

    #[test]
    fn preload_carries_its_own_gain() {
        let preload = Op::Preload {
            url: "u".into(),
            filters: gain(Some(-3.0)),
        };
        assert_eq!(
            commands(&preload),
            vec![json!({
                "name": "loadfile",
                "url": "u",
                "flags": "append",
                "options": "af=volume=volume=-3.00dB"
            })]
        );
        let plain = Op::Preload {
            url: "u".into(),
            filters: gain(None),
        };
        assert_eq!(
            commands(&plain),
            vec![json!({ "name": "loadfile", "url": "u", "flags": "append" })]
        );
    }

    #[test]
    fn window_and_transport_commands() {
        assert_eq!(
            commands(&Op::RemoveNext),
            vec![json!(["playlist-remove", 1])]
        );
        assert_eq!(
            commands(&Op::DropPrevious),
            vec![json!(["playlist-remove", 0])]
        );
        assert_eq!(
            commands(&Op::Seek(12.5)),
            vec![json!(["seek", 12.5, "absolute"])]
        );
        assert_eq!(
            commands(&Op::LoopCurrent(true)),
            vec![json!(["set_property", "loop-file", "inf"])]
        );
        assert_eq!(commands(&Op::LoopCurrent(false))[0][2], json!("no"));
        assert_eq!(
            commands(&Op::Volume {
                level: 60,
                muted: true
            }),
            vec![
                json!(["set_property", "volume", 60]),
                json!(["set_property", "mute", true]),
            ]
        );
        assert_eq!(
            commands(&Op::CurrentFilters(gain(None))),
            vec![json!(["set_property", "file-local-options/af", ""])]
        );
        assert_eq!(commands(&Op::CacheSize(CacheSize::Mib64)).len(), 2);
        assert_eq!(
            commands(&Op::Exclusive(true)),
            vec![json!(["set_property", "audio-exclusive", true])]
        );
    }

    #[test]
    fn events_are_translated() {
        let prop = |p| translate(MpvEvent::Property(p));
        assert_eq!(prop(Property::PlaylistPos(Some(1))), Some(Event::Advanced));
        assert_eq!(prop(Property::PlaylistPos(Some(0))), None);
        assert_eq!(prop(Property::PlaylistPos(None)), None);
        assert_eq!(
            prop(Property::TimePos(Some(3.0))),
            Some(Event::Position(Some(3.0)))
        );
        assert_eq!(translate(MpvEvent::StartFile), None);
        assert_eq!(translate(MpvEvent::FileLoaded), Some(Event::Loaded));
        assert_eq!(translate(MpvEvent::Disconnected), Some(Event::Stopped));
    }

    #[test]
    fn end_reasons() {
        let end = |reason, error: Option<&str>| {
            translate(MpvEvent::EndFile {
                reason,
                error: error.map(str::to_owned),
            })
        };
        assert_eq!(end(EndReason::Eof, None), Some(Event::Ended(End::Eof)));
        assert_eq!(
            end(EndReason::Error, Some(INIT_FAILED)),
            Some(Event::Ended(End::OutputFailed))
        );
        assert_eq!(
            end(EndReason::Error, Some("loading failed")),
            Some(Event::Ended(End::Error(Some("loading failed".into()))))
        );
        assert_eq!(end(EndReason::Stop, None), None);
        assert_eq!(end(EndReason::Redirect, None), None);
    }

    /// Frozen mpv: the operation fails after the timeout instead of blocking the actor (#255).
    #[tokio::test]
    async fn unresponsive_mpv_times_out() {
        let engine = MpvEngine {
            handle: MpvHandle::unresponsive(),
            process: Arc::new(ProcessGuard::default()),
        };
        let start = std::time::Instant::now();
        let result = engine
            .apply_within(Op::SetPaused(true), Duration::from_millis(50))
            .await;
        assert!(result.is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    /// Real mpv stopped by `terminate` (timed out): the actor receives `Event::Stopped`,
    /// which triggers the restart, and the guard does not think the app is closing (#255).
    /// `cargo test live_engine_terminate -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "starts a real mpv"]
    async fn live_engine_terminate() {
        let conf = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("mpv/seaglass.conf");
        let guard = Arc::new(ProcessGuard::default());
        let (tx, mut events) = mpsc::channel(256);
        let engine = MpvEngine::start(&conf, CacheSize::Mib32, Output::default(), &guard, tx)
            .await
            .unwrap();
        engine.apply(Op::SetPaused(true)).await.unwrap();
        guard.terminate();
        let stopped = tokio::time::timeout(Duration::from_secs(5), async {
            while let Some(ev) = events.recv().await {
                if ev == Event::Stopped {
                    return true;
                }
            }
            false
        })
        .await
        .unwrap_or(false);
        assert!(stopped, "Event::Stopped expected");
        assert!(!guard.closing());
        assert!(engine.apply(Op::SetPaused(false)).await.is_err());
        guard.kill();
        assert!(guard.closing());
    }

    /// Real mpv, two short files at low volume: `Play`, `Preload`, moving on (`Advanced`),
    /// `DropPrevious`, then the natural end (`Ended(Eof)`), all seen through the event
    /// translation.
    /// `cargo test live_engine_window -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "starts a real mpv on the audio output"]
    async fn live_engine_window() {
        use std::time::Duration;

        use crate::player::normalize::tests::quiet_wav;

        let conf = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("mpv/seaglass.conf");
        let guard = Arc::new(ProcessGuard::default());
        let (tx, mut events) = mpsc::channel(256);
        let engine = MpvEngine::start(&conf, CacheSize::Mib32, Output::default(), &guard, tx)
            .await
            .unwrap();
        async fn wait_for(
            events: &mut mpsc::Receiver<Event>,
            pred: fn(&Event) -> bool,
            what: &str,
        ) -> Event {
            tokio::time::timeout(Duration::from_secs(8), async {
                while let Some(ev) = events.recv().await {
                    if pred(&ev) {
                        return ev;
                    }
                }
                panic!("channel closed before: {what}");
            })
            .await
            .unwrap_or_else(|_| panic!("timed out: {what}"))
        }
        let a = quiet_wav("engine-a", 2, 440.0);
        let b = quiet_wav("engine-b", 2, 660.0);
        let url = |p: &std::path::PathBuf| p.to_string_lossy().into_owned();

        engine
            .apply(Op::Volume {
                level: 30,
                muted: false,
            })
            .await
            .unwrap();
        engine
            .apply(Op::Play {
                url: url(&a),
                start: 0.0,
                filters: Filters {
                    gain: Some(-6.0),
                    eq: None,
                },
            })
            .await
            .unwrap();
        wait_for(&mut events, |e| *e == Event::Loaded, "track A loaded").await;
        engine
            .apply(Op::Preload {
                url: url(&b),
                filters: Filters::default(),
            })
            .await
            .unwrap();
        wait_for(&mut events, |e| *e == Event::Advanced, "moved on to B").await;
        engine.apply(Op::DropPrevious).await.unwrap();
        let format = wait_for(
            &mut events,
            |e| matches!(e, Event::Format(Some(_))),
            "format de B",
        )
        .await;
        println!("B: {format:?}");
        wait_for(&mut events, |e| *e == Event::Ended(End::Eof), "end of B").await;
        println!("window: Play → Preload → Advanced → DropPrevious → Ended(Eof)");

        guard.kill();
        for p in [a, b] {
            let _ = std::fs::remove_file(p);
        }
    }
}
