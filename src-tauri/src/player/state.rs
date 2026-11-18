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
    /// Format réellement écrit sur la sortie (#245) : mixage de Windows en partagé, format
    /// ouvert sur le périphérique en exclusif. `None` sur Android.
    pub device_format: Option<AudioParams>,
    pub output_driver: Option<String>,
    /// Mode exclusif WASAPI actif (#245).
    pub exclusive: bool,
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
    /// Position jusqu'où le titre courant est déjà en mémoire, en secondes (#453) ; 0 si
    /// inconnue. Arrondie à la seconde près par `set_cached`.
    pub cached: f64,
    /// Le titre suivant est préchargé dans le moteur (fenêtre de 2 entrées).
    pub next_ready: bool,
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
    /// Profil d'égaliseur appliqué (#428) ; `None` si l'égaliseur est désactivé.
    pub equalizer: Option<String>,
    /// Minuteur de mise en veille actif (#85).
    pub sleep: Option<SleepState>,
    /// mpv ne tourne pas (lancement raté ou arrêt), avec sa cause probable (#92). Dans
    /// l'état plutôt qu'en toast : l'échec au lancement précède l'écoute de la webview.
    pub mpv_issue: Option<MpvIssue>,
    /// Le titre courant vient d'une bande originale choisie (clin d'œil #399), recalculé à
    /// chaque émission.
    pub soundtrack: bool,
    /// La file s'est arrêtée d'elle-même, rien ne suit (clin d'œil #399) ; remis à faux au
    /// chargement suivant. `idle` ne suffit pas : il vaut aussi au démarrage.
    pub ended: bool,
    /// Partie de blind test en cours (#434) : sans `track` tant que la manche n'est pas
    /// révélée.
    pub blind_test: bool,
}

impl PlayerState {
    /// Vrai si seuls `time_pos` et `cached` diffèrent : ces changements sont limités à
    /// ~4 Hz. Sans copie de l'état (appelé à chaque position reçue, #147) : les deux champs
    /// sont échangés le temps de la comparaison.
    pub fn differs_only_by_time(&mut self, other: &Self) -> bool {
        if self.time_pos == other.time_pos && self.cached == other.cached {
            return false;
        }
        let own_time = std::mem::replace(&mut self.time_pos, other.time_pos);
        let own_cached = std::mem::replace(&mut self.cached, other.cached);
        let same = *self == *other;
        self.time_pos = own_time;
        self.cached = own_cached;
        same
    }

    /// Applique la position mise en mémoire (#453). Le cache se remplit par petits paquets :
    /// seuls un écart d'une seconde ou plus, une baisse (nouveau titre, saut) et la fin du
    /// titre sont gardés, pour ne pas réveiller l'UI à chaque paquet.
    pub fn set_cached(&mut self, end: Option<f64>) {
        let end = end.unwrap_or(0.0).max(0.0);
        let reaches_end = self.duration > 0.0 && end >= self.duration - 1.0;
        if end < self.cached || end - self.cached >= 1.0 || (reaches_end && end != self.cached) {
            self.cached = end;
        }
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

    /// Applique `audio-out-params`, avec la même règle que `set_output` : un `null` (fin de
    /// fichier, réouverture de la sortie) n'efface rien.
    pub fn set_device_format(&mut self, params: Option<AudioParams>) {
        if params.is_some() {
            self.device_format = params;
        }
    }

    /// Applique `idle-active` : sans fichier, mpv n'a plus de sortie audio ouverte.
    pub fn set_idle(&mut self, idle: bool) {
        self.idle = idle;
        if idle {
            self.output = None;
            self.device_format = None;
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

        // Le cache qui avance est limité comme la position.
        let d = PlayerState {
            time_pos: 1.0,
            cached: 30.0,
            ..a.clone()
        };
        assert!(a.differs_only_by_time(&d));
        assert_eq!(a, PlayerState::default());
    }

    #[test]
    fn cached_moves_by_whole_seconds() {
        let mut s = PlayerState {
            duration: 200.0,
            ..PlayerState::default()
        };
        s.set_cached(Some(10.0));
        assert_eq!(s.cached, 10.0);
        // Petit paquet : ignoré.
        s.set_cached(Some(10.6));
        assert_eq!(s.cached, 10.0);
        s.set_cached(Some(11.2));
        assert_eq!(s.cached, 11.2);
        // Fin du titre atteinte : gardée même à moins d'une seconde.
        s.set_cached(Some(198.8));
        s.set_cached(Some(199.5));
        assert_eq!(s.cached, 199.5);
        // Nouveau titre ou saut en arrière : toujours appliqué.
        s.set_cached(Some(3.0));
        assert_eq!(s.cached, 3.0);
        s.set_cached(None);
        assert_eq!(s.cached, 0.0);
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
