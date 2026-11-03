//! États envoyés à l'UI : `player-state` (fréquent, léger) et `queue-state`
//! (seulement quand la file change, car il contient des listes de titres).

use serde::Serialize;

use super::output::OutputDevice;
use super::queue::Repeat;
use super::sleep::SleepState;
use super::source::PlaySource;
use super::volume::Volume;
use crate::engine::AudioParams;
use crate::mpv::MpvIssue;
use crate::tidal::models::{StreamFormat, Track};

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    pub track: Option<Track>,
    /// Format servi par TIDAL.
    pub stream: Option<StreamFormat>,
    /// Format réellement envoyé à la sortie audio par mpv.
    pub output: Option<AudioParams>,
    pub output_driver: Option<String>,
    /// Périphérique de sortie réellement utilisé (#79).
    pub output_device: Option<OutputDevice>,
    /// Un titre est en cours de résolution (métadonnées + manifeste).
    pub loading: bool,
    pub paused: bool,
    pub buffering: bool,
    pub idle: bool,
    /// Position en secondes ; l'UI interpole entre deux événements.
    pub time_pos: f64,
    pub duration: f64,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub has_previous: bool,
    pub has_next: bool,
    /// Volume logiciel de mpv ; tout autre réglage que 100 non coupé n'est plus bit-perfect.
    pub volume: Volume,
    /// Édition hi-res proposée pour le titre courant (réglage « Demander », #59) : son
    /// format, l'album dans `hires_edition`.
    pub hires_offer: Option<StreamFormat>,
    /// Gain de normalisation appliqué au titre courant, en dB (#82) ; `None` si le signal
    /// n'est pas modifié (réglage désactivé, gain inconnu ou nul).
    pub normalization: Option<f64>,
    /// Minuteur de mise en veille actif (#85).
    pub sleep: Option<SleepState>,
    /// mpv ne tourne pas (lancement raté ou arrêt), avec sa cause probable (#92). Dans
    /// l'état plutôt qu'en toast : l'échec au lancement précède l'écoute de la webview.
    pub mpv_issue: Option<MpvIssue>,
}

impl PlayerState {
    /// Vrai si seul `time_pos` diffère : ces changements sont limités à ~4 Hz. Sans copie
    /// de l'état (appelé à chaque position reçue, #147) : `time_pos` est échangé le temps de
    /// la comparaison.
    pub fn differs_only_by_time(&mut self, other: &Self) -> bool {
        if self.time_pos == other.time_pos {
            return false;
        }
        let own = std::mem::replace(&mut self.time_pos, other.time_pos);
        let same = *self == *other;
        self.time_pos = own;
        same
    }

    /// Applique `audio-params`. mpv le passe à `null` à chaque fin de fichier, mais ne renvoie
    /// pas la valeur si le fichier suivant a le même format (la chaîne audio n'est pas
    /// reconfigurée) : un `null` est donc ignoré, la sortie n'est effacée qu'avec `set_idle`.
    /// Un changement de format, lui, est toujours notifié.
    pub fn set_output(&mut self, params: Option<AudioParams>) {
        if params.is_some() {
            self.output = params;
        }
    }

    /// Applique `idle-active` : sans fichier, mpv n'a plus de sortie audio ouverte.
    pub fn set_idle(&mut self, idle: bool) {
        self.idle = idle;
        if idle {
            self.output = None;
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueView {
    /// File manuelle (« Lire ensuite » / « Ajouter à la file »).
    pub manual: Vec<Track>,
    /// Prochains titres du contexte.
    pub upcoming: Vec<Track>,
    /// Titre de départ quand le contexte est une radio qui prolonge la file (#78).
    pub radio: Option<Track>,
    /// D'où vient le titre en cours (« Joué depuis », #124), s'il vient du contexte.
    pub source: Option<PlaySource>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_only_difference() {
        let mut a = PlayerState::default();
        let b = PlayerState {
            time_pos: 1.0,
            ..a.clone()
        };
        let c = PlayerState {
            time_pos: 1.0,
            paused: true,
            ..a.clone()
        };
        assert!(a.differs_only_by_time(&b));
        assert!(!a.differs_only_by_time(&c));
        let same = a.clone();
        assert!(!a.differs_only_by_time(&same));
        // L'état comparé est rendu intact.
        assert_eq!(a, PlayerState::default());
    }

    fn params(samplerate: u32) -> Option<AudioParams> {
        Some(AudioParams {
            format: "s32".into(),
            samplerate,
            channel_count: 2,
        })
    }

    /// Changement de titre de même format : mpv envoie `null` puis plus rien (#12).
    #[test]
    fn output_survives_track_change_with_same_format() {
        let mut s = PlayerState::default();
        s.set_idle(false);
        s.set_output(params(96000));
        s.set_output(None);
        assert_eq!(s.output, params(96000));

        // Un nouveau format remplace la sortie.
        s.set_output(params(44100));
        assert_eq!(s.output, params(44100));

        // Plus rien à lire : la sortie est fermée.
        s.set_idle(true);
        assert_eq!(s.output, None);
        assert!(s.idle);
    }
}
