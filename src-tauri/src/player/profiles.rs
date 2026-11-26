//! Audio settings kept per output device (#416): volume, normalization, exclusive mode and
//! equalizer profile (#428) of each device actually used (the choice, or with "system
//! default" the Windows device behind it), applied when the output changes.
//!
//! Each change is noted in the profile of the current device; a still-unknown device
//! inherits the settings in effect. Can be turned off (`device_profiles`): the settings then
//! stay global, and the profiles already noted are kept.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::Emitter;

use super::actor::Player;
use super::normalize::Normalization;
use super::settings::PlaybackSettings;
use super::volume::Volume;
use crate::store::Store;

/// Key of the `kv` table.
const KEY: &str = "playback.deviceProfiles";
/// Most devices kept: the least recently used are forgotten.
const MAX_PROFILES: usize = 32;

/// Settings kept for a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DeviceProfile {
    pub volume: Volume,
    pub normalization: Normalization,
    pub exclusive: bool,
    /// Equalizer profile applied (#428), `None` = equalizer off. Missing from the profiles
    /// noted before 0.5.81: off.
    #[serde(default)]
    pub equalizer: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Entry {
    /// Engine identifier (`wasapi/{guid}` with mpv).
    id: String,
    profile: DeviceProfile,
}

/// Profiles and the device the settings in effect refer to. Pure logic.
#[derive(Debug, Clone, Default)]
pub(super) struct Profiles {
    enabled: bool,
    /// From least to most recently used.
    entries: Vec<Entry>,
    /// `None`: unknown device (no list yet, or default output with no known device behind
    /// it), nothing is noted.
    device: Option<String>,
}

impl Profiles {
    pub(super) fn new(enabled: bool, entries: Vec<Entry>) -> Self {
        Self {
            enabled,
            entries,
            device: None,
        }
    }

    pub(super) fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Settings in effect changed: noted for the current device. Returns `true` if the
    /// profiles changed (to be saved).
    pub(super) fn record(&mut self, current: DeviceProfile) -> bool {
        if !self.enabled {
            return false;
        }
        let Some(id) = self.device.clone() else {
            return false;
        };
        self.upsert(&id, current)
    }

    /// The device in use is now `id`. Returns the profile to apply if it has one that differs
    /// from the settings in effect; an unknown device adopts them. The second element says
    /// whether the profiles changed.
    pub(super) fn switch(
        &mut self,
        id: Option<&str>,
        current: DeviceProfile,
    ) -> (Option<DeviceProfile>, bool) {
        if !self.enabled || self.device.as_deref() == id {
            return (None, false);
        }
        self.device = id.map(str::to_owned);
        let Some(id) = id else {
            return (None, false);
        };
        match self.entries.iter().position(|e| e.id == id) {
            Some(i) => {
                // Put back last (the most recent): to be saved if it was not there.
                let moved = i + 1 != self.entries.len();
                let entry = self.entries.remove(i);
                let profile = entry.profile;
                self.entries.push(entry);
                ((profile != current).then_some(profile), moved)
            }
            None => (None, self.upsert(id, current)),
        }
    }

    /// Turned on or off in Settings. When turned on, the settings in effect become the
    /// current device's profile (without applying anything). Returns `true` if the profiles
    /// changed.
    pub(super) fn set_enabled(
        &mut self,
        enabled: bool,
        id: Option<&str>,
        current: DeviceProfile,
    ) -> bool {
        if enabled == self.enabled {
            return false;
        }
        self.enabled = enabled;
        self.device = None;
        if !enabled {
            return false;
        }
        self.device = id.map(str::to_owned);
        self.record(current)
    }

    fn upsert(&mut self, id: &str, profile: DeviceProfile) -> bool {
        let last = self.entries.last().is_some_and(|e| e.id == id);
        if last && self.entries.last().is_some_and(|e| e.profile == profile) {
            return false;
        }
        if !last {
            self.entries.retain(|e| e.id != id);
            self.entries.push(Entry {
                id: id.to_owned(),
                profile,
            });
            let excess = self.entries.len().saturating_sub(MAX_PROFILES);
            self.entries.drain(..excess);
        } else if let Some(e) = self.entries.last_mut() {
            e.profile = profile;
        }
        true
    }
}

/// Saved profiles; none without a database or if the value is unreadable.
pub(super) fn load(store: Option<&Store>) -> Vec<Entry> {
    let Some(store) = store else {
        return Vec::new();
    };
    match store.get_setting::<Vec<Entry>>(KEY) {
        Ok(entries) => entries.unwrap_or_default(),
        Err(e) => {
            tracing::warn!("unreadable output profiles, forgotten: {e}");
            Vec::new()
        }
    }
}

pub(super) fn save(store: &Store, entries: &[Entry]) {
    if let Err(e) = store.set_setting(KEY, &entries) {
        tracing::warn!("cannot save the output profiles: {e}");
    }
}

impl Player {
    fn device_profile(&self) -> DeviceProfile {
        DeviceProfile {
            volume: self.state.volume,
            normalization: self.normalizer.mode,
            exclusive: self.state.exclusive,
            equalizer: self.equalizer.settings.device_choice(),
        }
    }

    /// Volume, normalization, exclusive mode or equalizer changed: noted for the current
    /// device.
    pub(super) fn note_device_profile(&mut self) {
        let current = self.device_profile();
        if self.profiles.record(current) {
            self.persist_profiles = true;
        }
    }

    /// `device_profiles` setting changed.
    pub(super) fn set_device_profiles(&mut self, enabled: bool) {
        let id = self.output.in_use();
        let current = self.device_profile();
        if self.profiles.set_enabled(enabled, id.as_deref(), current) {
            self.persist_profiles = true;
        }
    }

    /// Output possibly changed (list, choice, fallback): applies the profile of the device
    /// now in use. Normalization, exclusive mode and equalizer live in the playback
    /// settings: they are saved again, and the UI rereads them on `playback-settings`.
    pub(super) async fn sync_device_profile(&mut self) {
        let id = self.output.in_use();
        let current = self.device_profile();
        let (profile, changed) = self.profiles.switch(id.as_deref(), current);
        if changed {
            self.persist_profiles = true;
        }
        let Some(profile) = profile else {
            return;
        };
        tracing::info!("output profile applied");
        if profile.volume != self.state.volume {
            self.state.volume = profile.volume.sanitized();
            self.persist_volume = true;
            self.apply_volume().await;
        }
        self.set_normalization(profile.normalization).await;
        let mut equalizer = self.equalizer.settings.clone();
        equalizer.apply_device_choice(profile.equalizer);
        self.set_equalizer(equalizer.clone()).await;
        self.set_exclusive(profile.exclusive).await;
        if profile.normalization == current.normalization
            && profile.exclusive == current.exclusive
            && profile.equalizer == current.equalizer
        {
            return;
        }
        let Some(store) = self.store.as_ref().map(Arc::clone) else {
            return;
        };
        let app = self.app.clone();
        tokio::task::spawn_blocking(move || {
            let mut settings = PlaybackSettings::load(Some(&store));
            settings.normalization = profile.normalization;
            settings.exclusive = profile.exclusive;
            // Only the switch and the chosen profile come from the device: the saved profiles
            // stay those of the settings.
            settings.equalizer.enabled = equalizer.enabled;
            settings.equalizer.active = equalizer.active;
            if let Err(e) = settings.save(&store) {
                tracing::warn!("playback settings not saved: {e}");
                return;
            }
            if let Err(e) = app.emit("playback-settings", ()) {
                tracing::warn!("emitting playback-settings: {e}");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAC: &str = "wasapi/{dac}";
    const HEADSET: &str = "wasapi/{headset}";

    fn profile(level: u8, normalization: Normalization, exclusive: bool) -> DeviceProfile {
        DeviceProfile {
            volume: Volume {
                level,
                muted: false,
            },
            normalization,
            exclusive,
            equalizer: None,
        }
    }

    #[test]
    fn each_device_keeps_its_settings() {
        let dac = profile(100, Normalization::Off, true);
        let headset = profile(40, Normalization::Track, false);
        let mut p = Profiles::new(true, Vec::new());

        // First device: it adopts the settings in effect.
        assert_eq!(p.switch(Some(DAC), dac), (None, true));
        // The headset too, then something else is set on it.
        assert_eq!(p.switch(Some(HEADSET), dac), (None, true));
        assert!(p.record(headset));
        assert!(!p.record(headset), "rien de neuf");

        // Back to the DAC: its settings return.
        assert_eq!(p.switch(Some(DAC), headset).0, Some(dac));
        // The same device applies nothing again.
        assert_eq!(p.switch(Some(DAC), dac), (None, false));
        // Back to the headset.
        assert_eq!(p.switch(Some(HEADSET), dac).0, Some(headset));
    }

    #[test]
    fn unknown_device_records_nothing() {
        let mut p = Profiles::new(true, Vec::new());
        let a = profile(70, Normalization::Off, false);
        assert!(!p.record(a), "no device yet");
        p.switch(Some(DAC), a);
        p.switch(None, a);
        assert!(!p.record(profile(10, Normalization::Album, false)));
        assert_eq!(p.switch(Some(DAC), a).0, None, "the DAC did not change");
    }

    #[test]
    fn disabled_profiles_stay_global() {
        let mut p = Profiles::new(false, Vec::new());
        let a = profile(70, Normalization::Off, false);
        assert_eq!(p.switch(Some(DAC), a), (None, false));
        assert!(!p.record(a));
        assert!(p.entries().is_empty());

        // Turn on: the settings in effect become the current device's, without overwriting
        // the others.
        let mut p = Profiles::new(
            false,
            vec![Entry {
                id: HEADSET.into(),
                profile: profile(30, Normalization::Track, false),
            }],
        );
        assert!(p.set_enabled(true, Some(DAC), a));
        assert_eq!(p.entries().len(), 2);
        assert_eq!(
            p.switch(Some(HEADSET), a).0,
            Some(profile(30, Normalization::Track, false))
        );
        // Turn off, then back on elsewhere.
        assert!(!p.set_enabled(false, Some(HEADSET), a));
        assert!(!p.record(profile(5, Normalization::Off, false)));
    }

    #[test]
    fn launch_applies_the_saved_profile() {
        let saved = profile(100, Normalization::Off, true);
        let mut p = Profiles::new(
            true,
            vec![Entry {
                id: DAC.into(),
                profile: saved,
            }],
        );
        let global = profile(40, Normalization::Track, false);
        assert_eq!(p.switch(Some(DAC), global), (Some(saved), false));
        assert_eq!(p.switch(Some(DAC), saved), (None, false), "same device");
    }

    #[test]
    fn oldest_devices_are_forgotten() {
        let a = profile(50, Normalization::Off, false);
        let mut p = Profiles::new(true, Vec::new());
        for i in 0..MAX_PROFILES + 3 {
            p.switch(Some(&format!("wasapi/{{{i}}}")), a);
        }
        assert_eq!(p.entries().len(), MAX_PROFILES);
        assert_eq!(p.entries()[0].id, "wasapi/{3}");
        // Reusing the oldest puts it back last.
        p.switch(Some("wasapi/{3}"), a);
        assert_eq!(
            p.entries().last().map(|e| e.id.as_str()),
            Some("wasapi/{3}")
        );
    }

    #[test]
    fn serialized_form() {
        let entries = vec![Entry {
            id: DAC.into(),
            profile: profile(100, Normalization::Off, true),
        }];
        assert_eq!(
            serde_json::to_string(&entries).unwrap(),
            r#"[{"id":"wasapi/{dac}","profile":{"volume":{"level":100,"muted":false},"normalization":"off","exclusive":true,"equalizer":null}}]"#
        );
    }

    /// Profiles noted before the equalizer (#428): reread, equalizer off.
    #[test]
    fn profiles_without_equalizer_are_kept() {
        let old = r#"[{"id":"wasapi/{dac}","profile":{"volume":{"level":100,"muted":false},"normalization":"off","exclusive":true}}]"#;
        let entries: Vec<Entry> = serde_json::from_str(old).unwrap();
        assert_eq!(entries[0].profile, profile(100, Normalization::Off, true));
    }

    #[test]
    fn each_device_keeps_its_equalizer() {
        let dac = profile(100, Normalization::Off, true);
        let headset = DeviceProfile {
            equalizer: Some(3),
            ..profile(40, Normalization::Off, false)
        };
        let mut p = Profiles::new(true, Vec::new());
        p.switch(Some(DAC), dac);
        p.switch(Some(HEADSET), dac);
        assert!(p.record(headset));
        assert_eq!(p.switch(Some(DAC), headset).0, Some(dac));
        assert_eq!(
            p.switch(Some(HEADSET), dac).0.and_then(|p| p.equalizer),
            Some(3)
        );
    }
}
