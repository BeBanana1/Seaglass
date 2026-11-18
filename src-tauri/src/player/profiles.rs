//! Réglages audio retenus par périphérique de sortie (#416) : volume, normalisation, mode
//! exclusif et profil d'égaliseur (#428) de chaque périphérique réellement utilisé (le choix, ou en « par défaut du
//! système » le périphérique Windows derrière), appliqués quand la sortie change.
//!
//! Chaque changement est noté dans le profil du périphérique en cours ; un périphérique
//! encore inconnu hérite des réglages en vigueur. Désactivable (`device_profiles`) : les
//! réglages restent alors globaux, et les profils déjà notés sont gardés.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::Emitter;

use super::actor::Player;
use super::normalize::Normalization;
use super::settings::PlaybackSettings;
use super::volume::Volume;
use crate::store::Store;

/// Clé de la table `kv`.
const KEY: &str = "playback.deviceProfiles";
/// Périphériques retenus au plus : les moins récemment utilisés sont oubliés.
const MAX_PROFILES: usize = 32;

/// Réglages retenus pour un périphérique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DeviceProfile {
    pub volume: Volume,
    pub normalization: Normalization,
    pub exclusive: bool,
    /// Profil d'égaliseur appliqué (#428), `None` = égaliseur désactivé. Absent des
    /// profils notés avant la 0.5.81 : désactivé.
    #[serde(default)]
    pub equalizer: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Entry {
    /// Identifiant du moteur (`wasapi/{guid}` sous mpv).
    id: String,
    profile: DeviceProfile,
}

/// Profils et périphérique auquel se rapportent les réglages en vigueur. Logique pure.
#[derive(Debug, Clone, Default)]
pub(super) struct Profiles {
    enabled: bool,
    /// Du moins au plus récemment utilisé.
    entries: Vec<Entry>,
    /// `None` : périphérique inconnu (pas encore de liste, ou sortie par défaut sans
    /// périphérique connu derrière), rien n'est noté.
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

    /// Réglages en vigueur changés : notés pour le périphérique en cours. Renvoie `true`
    /// si les profils ont changé (à enregistrer).
    pub(super) fn record(&mut self, current: DeviceProfile) -> bool {
        if !self.enabled {
            return false;
        }
        let Some(id) = self.device.clone() else {
            return false;
        };
        self.upsert(&id, current)
    }

    /// Le périphérique utilisé est maintenant `id`. Renvoie le profil à appliquer s'il en a
    /// un qui diffère des réglages en vigueur ; un périphérique inconnu les adopte.
    /// Le second élément dit si les profils ont changé.
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
                // Remis en dernier (le plus récent) : à enregistrer s'il n'y était pas.
                let moved = i + 1 != self.entries.len();
                let entry = self.entries.remove(i);
                let profile = entry.profile;
                self.entries.push(entry);
                ((profile != current).then_some(profile), moved)
            }
            None => (None, self.upsert(id, current)),
        }
    }

    /// Activé ou désactivé dans les Réglages. À l'activation, les réglages en vigueur
    /// deviennent le profil du périphérique en cours (sans rien appliquer). Renvoie `true`
    /// si les profils ont changé.
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

/// Profils enregistrés ; aucun sans base ou si la valeur est illisible.
pub(super) fn load(store: Option<&Store>) -> Vec<Entry> {
    let Some(store) = store else {
        return Vec::new();
    };
    match store.get_setting::<Vec<Entry>>(KEY) {
        Ok(entries) => entries.unwrap_or_default(),
        Err(e) => {
            tracing::warn!("profils de sortie illisibles, oubliés : {e}");
            Vec::new()
        }
    }
}

pub(super) fn save(store: &Store, entries: &[Entry]) {
    if let Err(e) = store.set_setting(KEY, &entries) {
        tracing::warn!("sauvegarde des profils de sortie impossible : {e}");
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

    /// Volume, normalisation, mode exclusif ou égaliseur changé : noté pour le périphérique en cours.
    pub(super) fn note_device_profile(&mut self) {
        let current = self.device_profile();
        if self.profiles.record(current) {
            self.persist_profiles = true;
        }
    }

    /// Réglage `device_profiles` changé.
    pub(super) fn set_device_profiles(&mut self, enabled: bool) {
        let id = self.output.in_use();
        let current = self.device_profile();
        if self.profiles.set_enabled(enabled, id.as_deref(), current) {
            self.persist_profiles = true;
        }
    }

    /// Sortie peut-être changée (liste, choix, repli) : applique le profil du périphérique
    /// qui sert désormais. Normalisation, mode exclusif et égaliseur vivent dans les
    /// réglages de lecture : ils sont réenregistrés, et l'UI les relit sur
    /// `playback-settings`.
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
        tracing::info!("profil de sortie appliqué");
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
            // Seuls l'interrupteur et le profil choisi viennent du périphérique : les profils
            // enregistrés restent ceux des réglages.
            settings.equalizer.enabled = equalizer.enabled;
            settings.equalizer.active = equalizer.active;
            if let Err(e) = settings.save(&store) {
                tracing::warn!("réglages de lecture non enregistrés : {e}");
                return;
            }
            if let Err(e) = app.emit("playback-settings", ()) {
                tracing::warn!("émission playback-settings : {e}");
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

        // Premier périphérique : il adopte les réglages en vigueur.
        assert_eq!(p.switch(Some(DAC), dac), (None, true));
        // Le casque aussi, puis on y règle autre chose.
        assert_eq!(p.switch(Some(HEADSET), dac), (None, true));
        assert!(p.record(headset));
        assert!(!p.record(headset), "rien de neuf");

        // Retour au DAC : ses réglages reviennent.
        assert_eq!(p.switch(Some(DAC), headset).0, Some(dac));
        // Le même périphérique ne réapplique rien.
        assert_eq!(p.switch(Some(DAC), dac), (None, false));
        // Retour au casque.
        assert_eq!(p.switch(Some(HEADSET), dac).0, Some(headset));
    }

    #[test]
    fn unknown_device_records_nothing() {
        let mut p = Profiles::new(true, Vec::new());
        let a = profile(70, Normalization::Off, false);
        assert!(!p.record(a), "pas encore de périphérique");
        p.switch(Some(DAC), a);
        p.switch(None, a);
        assert!(!p.record(profile(10, Normalization::Album, false)));
        assert_eq!(p.switch(Some(DAC), a).0, None, "le DAC n'a pas bougé");
    }

    #[test]
    fn disabled_profiles_stay_global() {
        let mut p = Profiles::new(false, Vec::new());
        let a = profile(70, Normalization::Off, false);
        assert_eq!(p.switch(Some(DAC), a), (None, false));
        assert!(!p.record(a));
        assert!(p.entries().is_empty());

        // Activer : les réglages en vigueur deviennent ceux du périphérique en cours,
        // sans écraser les autres.
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
        // Désactiver puis réactiver ailleurs.
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
        assert_eq!(
            p.switch(Some(DAC), saved),
            (None, false),
            "même périphérique"
        );
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
        // Réutiliser le plus ancien le remet en dernier.
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

    /// Profils notés avant l'égaliseur (#428) : relus, égaliseur désactivé.
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
