//! Moteur mpv : traduit les opérations de `engine::Op` en commandes JSON IPC et les
//! événements de mpv en `engine::Event`. Logique pure testée, sauf `MpvEngine::start`.

use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::device::INIT_FAILED;
use super::events::{EndReason, MpvEvent, Property};
use super::process::ProcessGuard;
use super::{CacheSize, MpvHandle};
use crate::engine::{End, Engine, Event, Op};

pub struct MpvEngine {
    handle: MpvHandle,
}

/// Ce qu'il faut pour lancer mpv : sa config dédiée et la garde du processus (arrêté à la
/// sortie de l'app et avant une mise à jour).
pub struct Launch {
    pub conf: std::path::PathBuf,
    pub process: std::sync::Arc<ProcessGuard>,
}

impl Launch {
    pub async fn start(
        &self,
        cache: CacheSize,
        audio_device: Option<&str>,
        events: mpsc::Sender<Event>,
    ) -> Result<MpvEngine, String> {
        MpvEngine::start(&self.conf, cache, audio_device, &self.process, events).await
    }
}

impl MpvEngine {
    /// Lance mpv (processus gardé par `process`), s'y connecte et transmet ses événements
    /// traduits à `events`.
    pub async fn start(
        conf: &std::path::Path,
        cache: CacheSize,
        audio_device: Option<&str>,
        process: &ProcessGuard,
        events: mpsc::Sender<Event>,
    ) -> Result<Self, String> {
        process.set(super::process::spawn(conf, cache, audio_device).map_err(|e| e.to_string())?);
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
        Ok(Self { handle })
    }
}

impl Engine for MpvEngine {
    /// Envoie les commandes de l'opération dans l'ordre, jusqu'à la première refusée.
    async fn apply(&self, op: Op) -> Result<(), String> {
        for command in commands(&op) {
            self.handle
                .command(command)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

/// Commandes mpv d'une opération.
pub fn commands(op: &Op) -> Vec<Vec<Value>> {
    let set = |name: &str, value: Value| vec![json!("set_property"), json!(name), value];
    let load = |url: &str, mode: &str, options: Option<String>| {
        let mut cmd = vec![json!("loadfile"), json!(url), json!(mode)];
        if let Some(options) = options {
            cmd.extend([json!(-1), json!(options)]);
        }
        cmd
    };
    match op {
        Op::Play { url, start, gain } => vec![
            load(url, "replace", file_options(*start, *gain)),
            set("pause", json!(false)),
        ],
        Op::Preload { url, gain } => vec![load(url, "append", file_options(0.0, *gain))],
        Op::RemoveNext => vec![vec![json!("playlist-remove"), json!(1)]],
        Op::DropPrevious => vec![vec![json!("playlist-remove"), json!(0)]],
        Op::Stop => vec![vec![json!("stop")]],
        Op::TogglePause => vec![vec![json!("cycle"), json!("pause")]],
        Op::SetPaused(paused) => vec![set("pause", json!(paused))],
        Op::Seek(secs) => vec![vec![json!("seek"), json!(secs), json!("absolute")]],
        Op::LoopCurrent(on) => vec![set("loop-file", json!(if *on { "inf" } else { "no" }))],
        Op::Volume { level, muted } => {
            vec![set("volume", json!(level)), set("mute", json!(muted))]
        }
        Op::CacheSize(size) => size.set_property_commands().into(),
        // Option locale du fichier : remise à la fin du titre, le préchargé garde la sienne.
        Op::CurrentGain(gain) => vec![set("file-local-options/af", json!(af_value(*gain)))],
        Op::OutputDevice(id) => vec![set("audio-device", json!(id))],
    }
}

/// Événement de mpv vu par l'acteur ; `None` s'il ne l'intéresse pas.
pub fn translate(ev: MpvEvent) -> Option<Event> {
    Some(match ev {
        MpvEvent::Property(p) => match p {
            // Fenêtre de 2 entrées : passer à l'entrée 1, c'est enchaîner sur le préchargé.
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

/// Valeur de `af` pour un gain (`volume=volume=-6.53dB`, filtre `volume` de libavfilter).
pub fn af_value(gain: Option<f64>) -> String {
    gain.map(|g| format!("volume=volume={g:.2}dB"))
        .unwrap_or_default()
}

/// Options de fichier du `loadfile` (4ᵉ argument, séparées par des virgules) : position de
/// départ et filtre de gain. `None` s'il n'y en a aucune.
pub fn file_options(start: f64, gain: Option<f64>) -> Option<String> {
    let mut options = Vec::new();
    if start > 0.0 {
        options.push(format!("start={start}"));
    }
    if gain.is_some() {
        options.push(format!("af={}", af_value(gain)));
    }
    (!options.is_empty()).then(|| options.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_options_syntax() {
        assert_eq!(af_value(Some(-6.5)), "volume=volume=-6.50dB");
        assert_eq!(af_value(None), "");
        assert_eq!(file_options(0.0, None), None);
        assert_eq!(file_options(12.5, None).as_deref(), Some("start=12.5"));
        assert_eq!(
            file_options(12.5, Some(3.1)).as_deref(),
            Some("start=12.5,af=volume=volume=3.10dB")
        );
        assert_eq!(
            file_options(0.0, Some(-1.0)).as_deref(),
            Some("af=volume=volume=-1.00dB")
        );
    }

    #[test]
    fn play_replaces_and_unpauses() {
        let play = Op::Play {
            url: "http://x/a.mpd".into(),
            start: 0.0,
            gain: None,
        };
        assert_eq!(
            commands(&play),
            vec![
                vec![json!("loadfile"), json!("http://x/a.mpd"), json!("replace")],
                vec![json!("set_property"), json!("pause"), json!(false)],
            ]
        );
        let resumed = Op::Play {
            url: "u".into(),
            start: 42.0,
            gain: Some(-6.5),
        };
        assert_eq!(
            commands(&resumed)[0],
            vec![
                json!("loadfile"),
                json!("u"),
                json!("replace"),
                json!(-1),
                json!("start=42,af=volume=volume=-6.50dB")
            ]
        );
    }

    #[test]
    fn preload_carries_its_own_gain() {
        let preload = Op::Preload {
            url: "u".into(),
            gain: Some(-3.0),
        };
        assert_eq!(
            commands(&preload),
            vec![vec![
                json!("loadfile"),
                json!("u"),
                json!("append"),
                json!(-1),
                json!("af=volume=volume=-3.00dB")
            ]]
        );
        let plain = Op::Preload {
            url: "u".into(),
            gain: None,
        };
        assert_eq!(
            commands(&plain),
            vec![vec![json!("loadfile"), json!("u"), json!("append")]]
        );
    }

    #[test]
    fn window_and_transport_commands() {
        assert_eq!(
            commands(&Op::RemoveNext),
            vec![vec![json!("playlist-remove"), json!(1)]]
        );
        assert_eq!(
            commands(&Op::DropPrevious),
            vec![vec![json!("playlist-remove"), json!(0)]]
        );
        assert_eq!(
            commands(&Op::Seek(12.5)),
            vec![vec![json!("seek"), json!(12.5), json!("absolute")]]
        );
        assert_eq!(
            commands(&Op::LoopCurrent(true)),
            vec![vec![
                json!("set_property"),
                json!("loop-file"),
                json!("inf")
            ]]
        );
        assert_eq!(commands(&Op::LoopCurrent(false))[0][2], json!("no"));
        assert_eq!(
            commands(&Op::Volume {
                level: 60,
                muted: true
            }),
            vec![
                vec![json!("set_property"), json!("volume"), json!(60)],
                vec![json!("set_property"), json!("mute"), json!(true)],
            ]
        );
        assert_eq!(
            commands(&Op::CurrentGain(None)),
            vec![vec![
                json!("set_property"),
                json!("file-local-options/af"),
                json!("")
            ]]
        );
        assert_eq!(commands(&Op::CacheSize(CacheSize::Mib64)).len(), 2);
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

    /// Vrai mpv, deux fichiers courts à volume bas : `Play`, `Preload`, enchaînement
    /// (`Advanced`), `DropPrevious`, puis fin naturelle (`Ended(Eof)`), le tout vu à travers
    /// la traduction des événements.
    /// `cargo test live_engine_window -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "lance un vrai mpv sur la sortie audio"]
    async fn live_engine_window() {
        use std::time::Duration;

        use crate::player::normalize::tests::quiet_wav;

        let conf = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("mpv/seaglass.conf");
        let guard = ProcessGuard::default();
        let (tx, mut events) = mpsc::channel(256);
        let engine = MpvEngine::start(&conf, CacheSize::Mib32, None, &guard, tx)
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
                panic!("canal fermé avant : {what}");
            })
            .await
            .unwrap_or_else(|_| panic!("délai dépassé : {what}"))
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
                gain: Some(-6.0),
            })
            .await
            .unwrap();
        wait_for(&mut events, |e| *e == Event::Loaded, "titre A chargé").await;
        engine
            .apply(Op::Preload {
                url: url(&b),
                gain: None,
            })
            .await
            .unwrap();
        wait_for(&mut events, |e| *e == Event::Advanced, "passage sur B").await;
        engine.apply(Op::DropPrevious).await.unwrap();
        let format = wait_for(
            &mut events,
            |e| matches!(e, Event::Format(Some(_))),
            "format de B",
        )
        .await;
        println!("B : {format:?}");
        wait_for(&mut events, |e| *e == Event::Ended(End::Eof), "fin de B").await;
        println!("fenêtre : Play → Preload → Advanced → DropPrevious → Ended(Eof)");

        guard.kill();
        for p in [a, b] {
            let _ = std::fs::remove_file(p);
        }
    }
}
