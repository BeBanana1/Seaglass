//! Audio output device (#79): saved choice, mpv's list, and fallback to the system
//! default device when the choice is unplugged, without losing that choice.

use serde::Serialize;
use tauri::Emitter;

use super::actor::Player;
use crate::engine::Op;
use crate::i18n::trf;
use crate::mpv::device::{self, AUTO, AudioDevice};

/// List sent to Settings (`audio_devices`, `audio-devices` event).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevices {
    /// WASAPI devices known to mpv.
    pub devices: Vec<AudioDevice>,
    /// Identifier of the Windows default device, if known.
    pub system_default: Option<String>,
}

/// Output actually in use, for the signal path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export, export_to = "../../src/lib/bindings/")
)]
pub struct OutputDevice {
    /// Windows name of the device opened by mpv; `None` if unknown.
    pub name: Option<String>,
    /// Output on the system default device (no choice, or fallback).
    pub system_default: bool,
    /// Chosen device that is unavailable (unplugged, refused): its saved name.
    pub missing: Option<String>,
}

/// Output state, held by the actor. Pure logic: the engine operations are sent by the
/// `Player` methods below.
#[derive(Debug, Clone)]
pub(super) struct Output {
    chosen: Option<AudioDevice>,
    /// `None` until mpv sent its first list.
    list: Option<Vec<AudioDevice>>,
    system_default: Option<String>,
    /// Listed device that refused to open, forgotten at the next list.
    unusable: Option<String>,
    /// Current value of `audio-device` in mpv (also passed on the command line).
    applied: String,
    fallback: bool,
}

impl Output {
    pub(super) fn new(chosen: Option<AudioDevice>) -> Self {
        let applied = chosen
            .as_ref()
            .map_or_else(|| AUTO.to_owned(), |d| d.id.clone());
        Self {
            chosen,
            list: None,
            system_default: None,
            unusable: None,
            applied,
            fallback: false,
        }
    }

    /// Value of `--audio-device` when mpv is launched.
    pub(super) fn launch_device(&self) -> Option<&str> {
        self.chosen.as_ref().map(|d| d.id.as_str())
    }

    pub(super) fn devices(&self) -> AudioDevices {
        AudioDevices {
            devices: self.list.clone().unwrap_or_default(),
            system_default: self.system_default.clone(),
        }
    }

    /// Identifier of the device in use: the choice, or in `auto` the system's
    /// (unknown on Linux).
    pub(super) fn in_use(&self) -> Option<String> {
        if self.applied == AUTO {
            self.system_default.clone()
        } else {
            Some(self.applied.clone())
        }
    }

    /// New list from mpv. Returns the name of the device used until now (the choice, or the
    /// system's in `auto`) if it just disappeared from it: unplugged (#348).
    /// A change of the default device whose old one stays listed does not count.
    fn set_list(
        &mut self,
        list: Vec<AudioDevice>,
        system_default: Option<String>,
    ) -> Option<String> {
        let lost = self
            .in_use()
            .and_then(|id| device::find(self.list.as_deref()?, &id))
            .filter(|d| device::find(&list, &d.id).is_none())
            .map(|d| d.name.clone());
        self.list = Some(list);
        self.system_default = system_default;
        self.unusable = None;
        lost
    }

    /// Recomputes the device to ask for. Returns the new value of `audio-device` if it
    /// changes, and the switch to fallback (`Some(true)`) or its end (`Some(false)`).
    fn update(&mut self) -> (Option<String>, Option<bool>) {
        let resolved = device::resolve(
            self.chosen.as_ref(),
            self.list.as_deref(),
            self.unusable.as_deref(),
        );
        let changed = (resolved.device != self.applied).then(|| resolved.device.clone());
        self.applied = resolved.device;
        let transition = (resolved.fallback != self.fallback).then_some(resolved.fallback);
        self.fallback = resolved.fallback;
        (changed, transition)
    }

    pub(super) fn describe(&self) -> OutputDevice {
        let list = self.list.as_deref().unwrap_or(&[]);
        let auto = self.applied == AUTO;
        let id = if auto {
            self.system_default.as_deref()
        } else {
            Some(self.applied.as_str())
        };
        let name = id
            .and_then(|id| device::find(list, id))
            .map(|d| d.name.clone())
            .or_else(|| {
                // Choice not confirmed by the list yet: saved name.
                (!auto)
                    .then(|| self.chosen.as_ref().map(|d| d.name.clone()))
                    .flatten()
            });
        OutputDevice {
            name,
            system_default: auto,
            missing: self
                .fallback
                .then(|| self.chosen.as_ref().map(|d| d.name.clone()))
                .flatten(),
        }
    }

    fn chosen_name(&self) -> String {
        self.chosen
            .as_ref()
            .map(|d| d.name.clone())
            .unwrap_or_default()
    }
}

impl Player {
    /// New list from mpv (startup, plugging, change of the default device).
    pub(super) async fn on_device_list(&mut self, list: Vec<AudioDevice>) {
        let system_default = tokio::task::spawn_blocking(device::system_default_id)
            .await
            .ok()
            .flatten();
        let lost = self.output.set_list(list, system_default);
        let devices = self.output.devices();
        if let Err(e) = self.app.emit("audio-devices", &devices) {
            tracing::warn!("cannot emit audio-devices: {e}");
        }
        // Unplugged during playback (#348): pause before switching the output, so that
        // nothing comes out of the speakers; this toast replaces the fallback one.
        let playing = self.loaded && !self.state.paused;
        match lost {
            Some(name) if playing && self.pause_on_unplug => {
                self.engine_op(Op::SetPaused(true)).await;
                self.apply_output_device(false).await;
                self.toast(
                    "info",
                    trf!(
                        "« {name} » débranché : lecture en pause",
                        "“{name}” unplugged: playback paused"
                    ),
                );
            }
            _ => self.apply_output_device(true).await,
        }
    }

    /// Choice made in Settings, applied live (mpv reopens the output without
    /// interrupting the track).
    pub(super) async fn set_audio_device(&mut self, chosen: Option<AudioDevice>) {
        if chosen == self.output.chosen {
            return;
        }
        self.output.chosen = chosen;
        self.output.unusable = None;
        self.apply_output_device(false).await;
    }

    /// The engine could not open the output: if it is the chosen device, fall back to
    /// the system's and resume the track at the same position. Returns `true` if the error
    /// is handled here (the URL retry is not used up).
    pub(super) async fn on_output_init_failed(&mut self) -> bool {
        if self.output.applied == AUTO {
            return false;
        }
        self.output.unusable = Some(self.output.applied.clone());
        self.apply_output_device(true).await;
        if let Some(id) = self.queue.current() {
            self.load_current(id, self.state.time_pos);
        }
        true
    }

    /// Sets `audio-device` if it changes and updates the signal path. `announce`: toast
    /// on switching to fallback and on coming back to the chosen device (not for a choice
    /// made in Settings).
    async fn apply_output_device(&mut self, announce: bool) {
        let (changed, transition) = self.output.update();
        if let Some(value) = changed {
            tracing::info!("audio output: {value}");
            self.engine_op(Op::OutputDevice(value)).await;
        }
        if announce && let Some(fallback) = transition {
            let name = self.output.chosen_name();
            let message = if fallback {
                trf!(
                    "« {name} » indisponible : sortie sur le périphérique par défaut",
                    "“{name}” unavailable: playing on the default device"
                )
            } else {
                trf!(
                    "Sortie de nouveau sur « {name} »",
                    "Playing on “{name}” again"
                )
            };
            self.toast("info", message);
        }
        self.state.output_device = Some(self.output.describe());
        self.sync_device_profile().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAC: &str = "wasapi/{dac}";
    const SPEAKERS: &str = "wasapi/{speakers}";

    fn dev(id: &str, name: &str) -> AudioDevice {
        AudioDevice {
            id: id.into(),
            name: name.into(),
        }
    }

    #[test]
    fn unplug_and_replug_keep_the_choice() {
        let mut out = Output::new(Some(dev(DAC, "DAC")));
        assert_eq!(out.launch_device(), Some(DAC));
        assert_eq!(out.describe().name.as_deref(), Some("DAC"));

        out.list = Some(vec![dev(SPEAKERS, "Haut-parleurs"), dev(DAC, "DAC USB")]);
        out.system_default = Some(SPEAKERS.into());
        assert_eq!(out.update(), (None, None));
        assert_eq!(
            out.describe(),
            OutputDevice {
                name: Some("DAC USB".into()),
                system_default: false,
                missing: None
            }
        );

        // Unplugged: system default, choice kept.
        out.list = Some(vec![dev(SPEAKERS, "Haut-parleurs")]);
        assert_eq!(out.update(), (Some(AUTO.into()), Some(true)));
        assert_eq!(
            out.describe(),
            OutputDevice {
                name: Some("Haut-parleurs".into()),
                system_default: true,
                missing: Some("DAC".into())
            }
        );
        assert_eq!(
            out.update(),
            (None, None),
            "rien ne change sans nouvelle liste"
        );

        // Plugged back.
        out.list = Some(vec![dev(SPEAKERS, "Haut-parleurs"), dev(DAC, "DAC USB")]);
        assert_eq!(out.update(), (Some(DAC.into()), Some(false)));
        assert_eq!(out.describe().missing, None);
    }

    #[test]
    fn unplugging_the_device_in_use_is_reported() {
        let speakers = || dev(SPEAKERS, "Haut-parleurs");
        let dac = || dev(DAC, "DAC USB");
        let both = || vec![speakers(), dac()];

        // Chosen device: the first list reports nothing, even when it is missing from it.
        let mut out = Output::new(Some(dev(DAC, "DAC")));
        assert_eq!(out.set_list(vec![speakers()], Some(SPEAKERS.into())), None);
        out.update();
        assert_eq!(out.set_list(both(), Some(SPEAKERS.into())), None);
        out.update();
        // Another device disappears: nothing.
        assert_eq!(out.set_list(vec![dac()], None), None);
        out.update();
        // The device in use disappears.
        assert_eq!(
            out.set_list(vec![speakers()], Some(SPEAKERS.into())),
            Some("DAC USB".into())
        );
        out.update();
        // Already in fallback: the next list does not report it a second time.
        assert_eq!(out.set_list(vec![speakers()], Some(SPEAKERS.into())), None);

        // System default device.
        let mut out = Output::new(None);
        assert_eq!(out.set_list(both(), Some(DAC.into())), None);
        out.update();
        // Default changed in Windows, the old one still plugged in: nothing.
        assert_eq!(out.set_list(both(), Some(SPEAKERS.into())), None);
        out.update();
        assert_eq!(out.set_list(both(), Some(DAC.into())), None);
        out.update();
        // Default headset unplugged: Windows moves to the speakers.
        assert_eq!(
            out.set_list(vec![speakers()], Some(SPEAKERS.into())),
            Some("DAC USB".into())
        );
        // Unknown default (Linux): nothing to report.
        let mut out = Output::new(None);
        assert_eq!(out.set_list(both(), None), None);
        assert_eq!(out.set_list(vec![speakers()], None), None);
    }

    #[test]
    fn refused_device_falls_back() {
        let mut out = Output::new(Some(dev(DAC, "DAC")));
        out.list = Some(vec![dev(DAC, "DAC")]);
        out.update();
        out.unusable = Some(DAC.into());
        assert_eq!(out.update(), (Some(AUTO.into()), Some(true)));
        // Without a known default device, the name stays unknown.
        assert_eq!(out.describe().name, None);
        assert!(out.describe().system_default);
    }

    #[test]
    fn system_default_choice() {
        let mut out = Output::new(None);
        assert_eq!(out.launch_device(), None);
        out.list = Some(vec![dev(SPEAKERS, "Haut-parleurs")]);
        out.system_default = Some(SPEAKERS.into());
        assert_eq!(out.update(), (None, None));
        assert_eq!(
            out.describe(),
            OutputDevice {
                name: Some("Haut-parleurs".into()),
                system_default: true,
                missing: None
            }
        );
        // Choose a device, then go back to the default.
        out.chosen = Some(dev(SPEAKERS, "Haut-parleurs"));
        assert_eq!(out.update(), (Some(SPEAKERS.into()), None));
        out.chosen = None;
        assert_eq!(out.update(), (Some(AUTO.into()), None));
    }
}
