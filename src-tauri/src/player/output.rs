//! Périphérique de sortie audio (#79) : choix enregistré, liste de mpv et repli sur le
//! périphérique par défaut du système quand le choix est débranché, sans perdre ce choix.

use serde::Serialize;
use tauri::Emitter;

use super::actor::Player;
use crate::engine::Op;
use crate::i18n::trf;
use crate::mpv::device::{self, AUTO, AudioDevice};

/// Liste envoyée aux Réglages (`audio_devices`, événement `audio-devices`).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevices {
    /// Périphériques WASAPI connus de mpv.
    pub devices: Vec<AudioDevice>,
    /// Identifiant du périphérique par défaut de Windows, s'il est connu.
    pub system_default: Option<String>,
}

/// Sortie réellement utilisée, pour la chaîne audio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputDevice {
    /// Nom Windows du périphérique ouvert par mpv ; `None` si inconnu.
    pub name: Option<String>,
    /// Sortie sur le périphérique par défaut du système (aucun choix, ou repli).
    pub system_default: bool,
    /// Périphérique choisi mais indisponible (débranché, refusé) : nom enregistré.
    pub missing: Option<String>,
}

/// État de la sortie, tenu par l'acteur. Logique pure : les opérations du moteur sont
/// envoyées par les méthodes de `Player` plus bas.
#[derive(Debug, Clone)]
pub(super) struct Output {
    chosen: Option<AudioDevice>,
    /// `None` tant que mpv n'a pas envoyé sa première liste.
    list: Option<Vec<AudioDevice>>,
    system_default: Option<String>,
    /// Périphérique listé qui a refusé de s'ouvrir, oublié à la liste suivante.
    unusable: Option<String>,
    /// Valeur actuelle de `audio-device` dans mpv (passée aussi en ligne de commande).
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

    /// Valeur de `--audio-device` au lancement de mpv.
    pub(super) fn launch_device(&self) -> Option<&str> {
        self.chosen.as_ref().map(|d| d.id.as_str())
    }

    pub(super) fn devices(&self) -> AudioDevices {
        AudioDevices {
            devices: self.list.clone().unwrap_or_default(),
            system_default: self.system_default.clone(),
        }
    }

    /// Identifiant du périphérique qui sert : le choix, ou en `auto` celui du système
    /// (inconnu sous Linux).
    pub(super) fn in_use(&self) -> Option<String> {
        if self.applied == AUTO {
            self.system_default.clone()
        } else {
            Some(self.applied.clone())
        }
    }

    /// Nouvelle liste de mpv. Renvoie le nom du périphérique qui servait jusque-là (le
    /// choix, ou celui du système en `auto`) s'il vient d'en disparaître : débranché (#348).
    /// Un changement du périphérique par défaut dont l'ancien reste listé ne compte pas.
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

    /// Recalcule le périphérique à demander. Renvoie la nouvelle valeur de `audio-device`
    /// si elle change, et le passage en repli (`Some(true)`) ou sa fin (`Some(false)`).
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
                // Choix pas encore confirmé par la liste : nom enregistré.
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
    /// Nouvelle liste de mpv (démarrage, branchement, changement du périphérique par défaut).
    pub(super) async fn on_device_list(&mut self, list: Vec<AudioDevice>) {
        let system_default = tokio::task::spawn_blocking(device::system_default_id)
            .await
            .ok()
            .flatten();
        let lost = self.output.set_list(list, system_default);
        let devices = self.output.devices();
        if let Err(e) = self.app.emit("audio-devices", &devices) {
            tracing::warn!("émission audio-devices : {e}");
        }
        // Débranché pendant la lecture (#348) : pause avant de basculer la sortie, pour
        // que rien ne sorte des haut-parleurs ; ce toast remplace celui du repli.
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

    /// Choix fait dans les Réglages, appliqué à chaud (mpv rouvre la sortie sans
    /// interrompre le titre).
    pub(super) async fn set_audio_device(&mut self, chosen: Option<AudioDevice>) {
        if chosen == self.output.chosen {
            return;
        }
        self.output.chosen = chosen;
        self.output.unusable = None;
        self.apply_output_device(false).await;
    }

    /// Le moteur n'a pas pu ouvrir la sortie : si c'est le périphérique choisi, repli sur
    /// celui du système et reprise du titre à la même position. Renvoie `true` si l'erreur
    /// est traitée ici (la nouvelle tentative d'URL n'est pas consommée).
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

    /// Pose `audio-device` s'il change et met à jour la chaîne audio. `announce` : toast
    /// au passage en repli et au retour sur le périphérique choisi (pas pour un choix fait
    /// dans les Réglages).
    async fn apply_output_device(&mut self, announce: bool) {
        let (changed, transition) = self.output.update();
        if let Some(value) = changed {
            tracing::info!("sortie audio : {value}");
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

        // Débranché : défaut du système, choix gardé.
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

        // Rebranché.
        out.list = Some(vec![dev(SPEAKERS, "Haut-parleurs"), dev(DAC, "DAC USB")]);
        assert_eq!(out.update(), (Some(DAC.into()), Some(false)));
        assert_eq!(out.describe().missing, None);
    }

    #[test]
    fn unplugging_the_device_in_use_is_reported() {
        let speakers = || dev(SPEAKERS, "Haut-parleurs");
        let dac = || dev(DAC, "DAC USB");
        let both = || vec![speakers(), dac()];

        // Périphérique choisi : la première liste ne signale rien, même s'il en est absent.
        let mut out = Output::new(Some(dev(DAC, "DAC")));
        assert_eq!(out.set_list(vec![speakers()], Some(SPEAKERS.into())), None);
        out.update();
        assert_eq!(out.set_list(both(), Some(SPEAKERS.into())), None);
        out.update();
        // Un autre périphérique disparaît : rien.
        assert_eq!(out.set_list(vec![dac()], None), None);
        out.update();
        // Le périphérique utilisé disparaît.
        assert_eq!(
            out.set_list(vec![speakers()], Some(SPEAKERS.into())),
            Some("DAC USB".into())
        );
        out.update();
        // Déjà en repli : la liste suivante ne le signale pas une seconde fois.
        assert_eq!(out.set_list(vec![speakers()], Some(SPEAKERS.into())), None);

        // Périphérique par défaut du système.
        let mut out = Output::new(None);
        assert_eq!(out.set_list(both(), Some(DAC.into())), None);
        out.update();
        // Défaut changé dans Windows, l'ancien toujours branché : rien.
        assert_eq!(out.set_list(both(), Some(SPEAKERS.into())), None);
        out.update();
        assert_eq!(out.set_list(both(), Some(DAC.into())), None);
        out.update();
        // Casque par défaut débranché : Windows passe aux haut-parleurs.
        assert_eq!(
            out.set_list(vec![speakers()], Some(SPEAKERS.into())),
            Some("DAC USB".into())
        );
        // Défaut inconnu (Linux) : rien à signaler.
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
        // Sans périphérique par défaut connu, le nom reste inconnu.
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
        // Choisir un périphérique puis revenir au défaut.
        out.chosen = Some(dev(SPEAKERS, "Haut-parleurs"));
        assert_eq!(out.update(), (Some(SPEAKERS.into()), None));
        out.chosen = None;
        assert_eq!(out.update(), (Some(AUTO.into()), None));
    }
}
