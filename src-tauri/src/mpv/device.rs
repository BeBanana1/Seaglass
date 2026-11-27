//! Audio output devices (#79): mpv's `audio-device-list`, the choice saved in the playback
//! settings and the device actually asked of mpv.
//!
//! Checked with mpv 0.41: the property can be observed (initial value sent right at
//! `observe_property`) and is notified again on WASAPI's hotplug events
//! (`ao_hotplug_event` in `ao_wasapi_changenotify.c`). An `audio-device` set during playback
//! reopens the output without interrupting the track. A device missing when a file loads
//! makes the file fail (`end-file` with an error, [`INIT_FAILED`]): mpv does not fall back
//! to the default device on its own, the player does.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Value of `audio-device` for the system's default device.
pub const AUTO: &str = "auto";
/// Output drivers passed to `--ao`, in order of preference. On Linux (#17), mpv takes the
/// first one that opens: PipeWire, then PulseAudio (WSLg, older desktops), then ALSA.
#[cfg(windows)]
pub const AO: &str = "wasapi";
#[cfg(not(windows))]
pub const AO: &str = "pipewire,pulse,alsa";
/// Identifier prefixes (`<driver>/…`) of the drivers in [`AO`], in the same order.
#[cfg(windows)]
const PREFIXES: &[&str] = &["wasapi/"];
#[cfg(not(windows))]
const PREFIXES: &[&str] = &["pipewire/", "pulse/", "alsa/"];
/// Prefixes accepted for a saved choice, on every platform: a choice from another
/// platform stays valid, it is only missing from the list (fallback).
const KNOWN_PREFIXES: &[&str] = &["wasapi/", "pipewire/", "pulse/", "alsa/"];
/// Bounds of a choice received from the UI: a WASAPI identifier is about forty characters.
const MAX_ID: usize = 200;
const MAX_NAME: usize = 200;
/// mpv's `file_error` when the audio output does not open (missing device…).
pub const INIT_FAILED: &str = "audio output initialization failed";

/// Output device: mpv identifier (`wasapi/{guid}`) and the name Windows shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawDevice")]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize)]
struct RawDevice {
    id: String,
    name: String,
}

/// Choice received from the UI or read back from the database: `<driver>/…` identifier
/// of a known driver, non-empty name, bounded lengths.
impl TryFrom<RawDevice> for AudioDevice {
    type Error = String;

    fn try_from(raw: RawDevice) -> Result<Self, Self::Error> {
        let name = raw.name.trim().to_owned();
        let valid_id = KNOWN_PREFIXES
            .iter()
            .any(|p| raw.id.starts_with(p) && raw.id.len() > p.len())
            && raw.id.len() <= MAX_ID
            && !raw.id.chars().any(char::is_control);
        if !valid_id {
            return Err(format!("invalid audio device: {:?}", raw.id));
        }
        if name.is_empty() || name.len() > MAX_NAME || name.chars().any(char::is_control) {
            return Err("invalid audio device name".into());
        }
        Ok(Self { id: raw.id, name })
    }
}

/// Entries of `audio-device-list` (`[{name, description}]`) for the driver in use, in
/// mpv's order. `auto` and the other outputs (`openal`…) are left out.
pub fn parse_list(data: &Value) -> Vec<AudioDevice> {
    pick(data, PREFIXES)
}

/// Keeps the entries of the first driver of `prefixes` present in the list. On Linux, mpv
/// lists the devices of all its drivers: PipeWire also shows them through its PulseAudio
/// and ALSA layers, so keeping everything would give duplicates.
fn pick(data: &Value, prefixes: &[&str]) -> Vec<AudioDevice> {
    let entries: Vec<&Value> = data.as_array().into_iter().flatten().collect();
    let id_of = |entry: &Value| entry.get("name").and_then(Value::as_str).map(str::to_owned);
    let Some(prefix) = prefixes.iter().find(|p| {
        entries
            .iter()
            .any(|e| id_of(e).is_some_and(|id| id.starts_with(**p) && id.len() > p.len()))
    }) else {
        return Vec::new();
    };
    entries
        .into_iter()
        .filter_map(|entry| {
            let id = entry.get("name")?.as_str()?;
            if !id.starts_with(prefix) || id.len() == prefix.len() {
                return None;
            }
            let name = entry
                .get("description")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .unwrap_or(id);
            Some(AudioDevice {
                id: id.to_owned(),
                name: name.to_owned(),
            })
        })
        .collect()
}

/// mpv identifier of a Windows endpoint: `IMMDevice::GetId` returns
/// `{0.0.0.00000000}.{guid}`, mpv only keeps the guid. Used on Windows only (dead code in
/// the Linux CI).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn mpv_id_from_endpoint(endpoint: &str) -> Option<String> {
    let guid = endpoint.rsplit('.').next()?;
    (guid.starts_with('{') && guid.ends_with('}') && guid.len() > 2)
        .then(|| format!("wasapi/{}", guid.to_ascii_lowercase()))
}

/// Finds an identifier in the list, ignoring the case of the guid.
pub fn find<'a>(list: &'a [AudioDevice], id: &str) -> Option<&'a AudioDevice> {
    list.iter().find(|d| d.id.eq_ignore_ascii_case(id))
}

/// Device to ask of mpv.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// Value of `audio-device`: the chosen identifier or [`AUTO`].
    pub device: String,
    /// The saved choice cannot be used: output on the default device.
    pub fallback: bool,
}

/// The saved choice if it is in mpv's list, otherwise the default device (fallback
/// reported). List not known yet (before the first notification): the choice is kept.
/// `unusable`: a device that refused to open although it was listed.
pub fn resolve(
    chosen: Option<&AudioDevice>,
    list: Option<&[AudioDevice]>,
    unusable: Option<&str>,
) -> Resolved {
    let Some(chosen) = chosen else {
        return Resolved {
            device: AUTO.into(),
            fallback: false,
        };
    };
    let present = list.is_none_or(|list| find(list, &chosen.id).is_some());
    let refused = unusable.is_some_and(|id| id.eq_ignore_ascii_case(&chosen.id));
    if present && !refused {
        Resolved {
            device: chosen.id.clone(),
            fallback: false,
        }
    } else {
        Resolved {
            device: AUTO.into(),
            fallback: true,
        }
    }
}

/// mpv identifier of Windows' default output device, to show its name when
/// `audio-device` is `auto`. Role `eMultimedia`, the same as mpv's
/// (`OnDefaultDeviceChanged` only listens to it). A blocking but short COM call.
#[cfg(windows)]
pub fn system_default_id() -> Option<String> {
    use windows::Win32::Media::Audio::{
        IMMDeviceEnumerator, MMDeviceEnumerator, eMultimedia, eRender,
    };
    use windows::Win32::System::Com::{
        CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    };

    // SAFETY: documented COM calls; the string returned by `GetId` is freed with
    // `CoTaskMemFree` after it is copied. An already initialized thread (S_FALSE, another
    // mode) stays usable: the result of `CoInitializeEx` is ignored.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, eMultimedia)
            .ok()?;
        let raw = device.GetId().ok()?;
        let text = raw.to_string().ok();
        CoTaskMemFree(Some(raw.0 as *const _));
        text.as_deref().and_then(mpv_id_from_endpoint)
    }
}

#[cfg(not(windows))]
pub fn system_default_id() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn device(id: &str, name: &str) -> AudioDevice {
        AudioDevice {
            id: id.into(),
            name: name.into(),
        }
    }

    const DAC: &str = "wasapi/{27264a86-c734-4c85-876c-dcfeb567529f}";

    #[test]
    fn list_keeps_wasapi_entries() {
        // Value read from mpv 0.41.
        let data = json!([
            {"name": "auto", "description": "Autoselect device"},
            {"name": DAC, "description": "Haut-parleurs (3- Focusrite USB Audio)"},
            {"name": "wasapi/{aaaa}", "description": "  "},
            {"name": "openal", "description": "Default (openal)"},
            {"description": "no name"}
        ]);
        assert_eq!(
            pick(&data, &["wasapi/"]),
            vec![
                device(DAC, "Haut-parleurs (3- Focusrite USB Audio)"),
                device("wasapi/{aaaa}", "wasapi/{aaaa}"),
            ]
        );
        assert!(parse_list(&Value::Null).is_empty());
    }

    #[test]
    fn list_keeps_the_first_linux_driver_present() {
        // PipeWire also lists its outputs through its PulseAudio layer and through ALSA.
        let data = json!([
            {"name": "auto", "description": "Autoselect device"},
            {"name": "pipewire", "description": "Default (pipewire)"},
            {"name": "pipewire/alsa_output.usb-dac", "description": "DAC USB"},
            {"name": "pulse/alsa_output.usb-dac", "description": "DAC USB"},
            {"name": "alsa/hw:CARD=DAC,DEV=0", "description": "DAC, USB Audio"},
        ]);
        let linux = ["pipewire/", "pulse/", "alsa/"];
        assert_eq!(
            pick(&data, &linux),
            vec![device("pipewire/alsa_output.usb-dac", "DAC USB")]
        );
        // WSLg: PulseAudio only.
        let wslg = json!([
            {"name": "pulse", "description": "Default (pulse)"},
            {"name": "pulse/RDPSink", "description": "RDP Sink"},
            {"name": "alsa/default", "description": "Default ALSA Output"},
        ]);
        assert_eq!(
            pick(&wslg, &linux),
            vec![device("pulse/RDPSink", "RDP Sink")]
        );
        assert!(pick(&json!([{"name": "auto"}]), &linux).is_empty());
    }

    #[test]
    fn choice_is_validated() {
        let ok: AudioDevice =
            serde_json::from_value(json!({"id": DAC, "name": " Focusrite "})).unwrap();
        assert_eq!(ok, device(DAC, "Focusrite"));
        for id in [
            "pipewire/alsa_output.usb",
            "pulse/RDPSink",
            "alsa/hw:CARD=DAC,DEV=0",
        ] {
            assert!(serde_json::from_value::<AudioDevice>(json!({"id": id, "name": "x"})).is_ok());
        }
        for bad in [
            json!({"id": "auto", "name": "x"}),
            json!({"id": "wasapi/", "name": "x"}),
            json!({"id": "openal", "name": "x"}),
            json!({"id": "pulse/", "name": "x"}),
            json!({"id": DAC, "name": "  "}),
            json!({"id": format!("wasapi/{}", "a".repeat(300)), "name": "x"}),
            json!({"id": "wasapi/{a}\n", "name": "x"}),
            json!({"id": DAC}),
        ] {
            assert!(
                serde_json::from_value::<AudioDevice>(bad.clone()).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn endpoint_id_maps_to_mpv() {
        assert_eq!(
            mpv_id_from_endpoint("{0.0.0.00000000}.{27264A86-C734-4C85-876C-DCFEB567529F}")
                .as_deref(),
            Some(DAC)
        );
        assert_eq!(mpv_id_from_endpoint("garbage"), None);
        assert_eq!(mpv_id_from_endpoint("{0.0.0.00000000}.{}"), None);
    }

    #[test]
    fn resolve_falls_back_without_losing_the_choice() {
        let dac = device(DAC, "Focusrite");
        let list = vec![dac.clone()];
        let auto = Resolved {
            device: AUTO.into(),
            fallback: false,
        };
        let on_dac = Resolved {
            device: DAC.into(),
            fallback: false,
        };
        let fallback = Resolved {
            device: AUTO.into(),
            fallback: true,
        };

        assert_eq!(resolve(None, Some(&list), None), auto);
        assert_eq!(resolve(None, None, None), auto);
        // List not received yet: the choice is followed (also passed on the command line).
        assert_eq!(resolve(Some(&dac), None, None), on_dac);
        assert_eq!(resolve(Some(&dac), Some(&list), None), on_dac);
        // Upper-case guid in the list: same device.
        let upper = vec![device(&DAC.to_uppercase().replace("WASAPI", "wasapi"), "F")];
        assert_eq!(resolve(Some(&dac), Some(&upper), None), on_dac);
        // Unplugged: system default; plugged back in: back to the choice.
        assert_eq!(resolve(Some(&dac), Some(&[]), None), fallback);
        assert_eq!(resolve(Some(&dac), Some(&list), None), on_dac);
        // Listed but refused by mpv: fallback too.
        assert_eq!(resolve(Some(&dac), Some(&list), Some(DAC)), fallback);
        assert_eq!(
            resolve(Some(&dac), Some(&list), Some("wasapi/{other}")),
            on_dac
        );
    }

    /// 16-bit stereo 44.1 kHz PCM WAV of silence (a test file, not a stream).
    fn silent_wav(secs: u32) -> std::path::PathBuf {
        let data_len: u32 = 44_100 * 4 * secs;
        let mut wav = Vec::with_capacity(44 + data_len as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        for (v, n) in [
            (16u32, 4),
            (1, 2),
            (2, 2),
            (44_100, 4),
            (44_100 * 4, 4),
            (4, 2),
            (16, 2),
        ] {
            wav.extend_from_slice(&v.to_le_bytes()[..n]);
        }
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        wav.resize(44 + data_len as usize, 0);
        let path = std::env::temp_dir().join(format!("seaglass-device-{}.wav", std::process::id()));
        std::fs::write(&path, &wav).unwrap();
        path
    }

    /// Real mpv on the real WASAPI output (silence): `audio-device-list` is notified as
    /// soon as it is observed and contains Windows' default device; a missing device makes
    /// the file fail with [`INIT_FAILED`]; changing `audio-device` during playback does not
    /// interrupt it.
    /// `cargo test live_audio_devices -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "starts a real mpv on the audio output"]
    async fn live_audio_devices() {
        use crate::mpv::CacheSize;
        use crate::mpv::events::{EndReason, MpvEvent, Property};
        use std::time::Duration;

        let conf = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("mpv/seaglass.conf");
        let bogus = "wasapi/{00000000-0000-0000-0000-000000000000}";
        let mut child =
            crate::mpv::process::spawn(&conf, CacheSize::Mib32, Some(bogus), false).unwrap();
        let (events_tx, mut events) = tokio::sync::mpsc::channel(256);
        let handle = crate::mpv::ipc::connect(events_tx).await.unwrap();
        let cmd = |c: Vec<Value>| {
            let handle = handle.clone();
            async move { handle.command(c).await }
        };
        async fn wait_for(
            events: &mut tokio::sync::mpsc::Receiver<MpvEvent>,
            pred: &dyn Fn(&MpvEvent) -> bool,
        ) -> MpvEvent {
            tokio::time::timeout(Duration::from_secs(10), async {
                while let Some(ev) = events.recv().await {
                    if pred(&ev) {
                        return ev;
                    }
                }
                panic!("mpv closed");
            })
            .await
            .expect("expected an event")
        }

        let MpvEvent::Property(Property::AudioDeviceList(list)) = wait_for(&mut events, &|ev| {
            matches!(ev, MpvEvent::Property(Property::AudioDeviceList(_)))
        })
        .await
        else {
            unreachable!()
        };
        println!("devices: {list:#?}");
        assert!(!list.is_empty(), "no WASAPI device");
        let default = system_default_id().expect("Windows default device");
        println!("Windows default: {default}");
        let default_dev = find(&list, &default).expect("default missing from mpv's list");

        // Device passed on the command line but missing: the file fails.
        let wav = silent_wav(6);
        cmd(vec![json!("loadfile"), json!(wav.to_string_lossy())])
            .await
            .unwrap();
        let ev = wait_for(&mut events, &|ev| matches!(ev, MpvEvent::EndFile { .. })).await;
        println!("absent: {ev:?}");
        assert_eq!(
            ev,
            MpvEvent::EndFile {
                reason: EndReason::Error,
                error: Some(INIT_FAILED.into())
            }
        );

        // Real device: playback, then a live switch to `auto`.
        cmd(vec![
            json!("set_property"),
            json!("audio-device"),
            json!(default_dev.id),
        ])
        .await
        .unwrap();
        cmd(vec![json!("loadfile"), json!(wav.to_string_lossy())])
            .await
            .unwrap();
        wait_for(&mut events, &|ev| {
            matches!(ev, MpvEvent::Property(Property::CurrentAo(Some(_))))
        })
        .await;
        tokio::time::sleep(Duration::from_millis(1500)).await;
        let pos = |v: Value| v.as_f64().unwrap_or(0.0);
        let before = pos(cmd(vec![json!("get_property"), json!("time-pos")])
            .await
            .unwrap());
        cmd(vec![
            json!("set_property"),
            json!("audio-device"),
            json!(AUTO),
        ])
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(1500)).await;
        let after = pos(cmd(vec![json!("get_property"), json!("time-pos")])
            .await
            .unwrap());
        println!("position before the switch {before:.2} s, after {after:.2} s");
        assert!(before > 0.5 && after > before + 0.5, "{before} → {after}");
        let ao = cmd(vec![json!("get_property"), json!("current-ao")])
            .await
            .unwrap();
        assert_eq!(ao, json!("wasapi"));

        let _ = child.start_kill();
        let _ = std::fs::remove_file(&wav);
    }
}
