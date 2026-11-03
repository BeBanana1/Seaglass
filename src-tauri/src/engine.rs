//! Contrat entre l'acteur du lecteur et le moteur qui décode le son : mpv sur le bureau,
//! Media3 sur Android à venir (#24).
//!
//! Le moteur tient une **fenêtre de 2 entrées au maximum** : le titre courant (0) et le
//! suivant préchargé (1). L'acteur décide de tout (file, préchargement, erreurs) ; le
//! moteur exécute des opérations sémantiques (`Op`) et remonte des événements (`Event`),
//! sans rien savoir de TIDAL ni de la file.
//!
//! Le périphérique de sortie (`AudioDevice`) et la taille du cache (`CacheSize`) restent
//! définis dans `mpv` : ce sont des réglages du bureau, à revoir avec le moteur Android.

// Sur Android, aucun moteur ne remonte encore d'événement : `End` n'y est pas construit.
#![cfg_attr(mobile, allow(dead_code))]

use serde::{Deserialize, Serialize};

use crate::mpv::{AudioDevice, CacheSize};

/// Opération demandée au moteur.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// Remplace toute la fenêtre par `url` (entrée 0) à partir de `start` secondes, avec
    /// son gain de normalisation, et lance la lecture.
    Play {
        url: String,
        start: f64,
        gain: Option<f64>,
    },
    /// Précharge `url` en entrée 1 pour un enchaînement sans blanc, avec son propre gain
    /// (le titre en cours garde le sien).
    Preload {
        url: String,
        gain: Option<f64>,
    },
    /// Retire le titre préchargé (la file a changé).
    RemoveNext,
    /// Oublie le titre terminé après `Event::Advanced` : la fenêtre redevient une entrée.
    DropPrevious,
    /// Arrête la lecture et vide la fenêtre.
    Stop,
    TogglePause,
    SetPaused(bool),
    /// Saut absolu, en secondes.
    Seek(f64),
    /// Lecture en boucle du titre courant (repeat One), sans rechargement.
    LoopCurrent(bool),
    /// Volume logiciel 0–100 (100 = gain unitaire) et coupure.
    Volume {
        level: u8,
        muted: bool,
    },
    CacheSize(CacheSize),
    /// Nouveau gain de normalisation du titre en cours seulement.
    CurrentGain(Option<f64>),
    /// Identifiant du périphérique de sortie (`auto` = défaut du système).
    OutputDevice(String),
}

/// Format décodé envoyé à la sortie (`audio-params` de mpv).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioParams {
    pub format: String,
    pub samplerate: u32,
    #[serde(rename(deserialize = "channel-count"))]
    pub channel_count: u32,
}

/// Fin d'un fichier qui intéresse l'acteur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    /// Fin naturelle du titre.
    Eof,
    /// Le périphérique de sortie n'a pas pu s'ouvrir : repli, pas une erreur du flux (#79).
    OutputFailed,
    /// Flux illisible (URL expirée, réseau…), avec le détail du moteur.
    Error(Option<String>),
}

/// Événement du moteur, déjà traduit.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Position(Option<f64>),
    Duration(Option<f64>),
    Paused(bool),
    Buffering(bool),
    /// `None` entre deux fichiers : sans effet sur l'affichage (#12).
    Format(Option<AudioParams>),
    /// Pilote de sortie (`wasapi`…).
    Driver(Option<String>),
    Idle(bool),
    /// Le moteur est passé sur l'entrée préchargée, sans blanc.
    Advanced,
    /// Fichier ouvert et décodable.
    Loaded,
    Ended(End),
    /// Périphériques de sortie connus, renvoyés au branchement (#79).
    Devices(Vec<AudioDevice>),
    /// Le moteur s'est arrêté (processus terminé ou planté).
    Stopped,
}

/// Ce qu'un moteur doit savoir faire. Une erreur est un message déjà traduit, affiché en
/// toast par l'acteur.
pub(crate) trait Engine {
    async fn apply(&self, op: Op) -> Result<(), String>;
}

#[cfg(desktop)]
pub use crate::mpv::engine::{Launch, MpvEngine as ActiveEngine};
#[cfg(mobile)]
pub use unavailable::{Launch, Unavailable as ActiveEngine};

/// Problème du moteur à signaler en bandeau après un échec de lancement ou un arrêt (#92).
pub fn diagnose() -> Option<crate::mpv::MpvIssue> {
    #[cfg(desktop)]
    return Some(crate::mpv::MpvIssue::diagnose());
    #[cfg(mobile)]
    None
}

/// Android avant le moteur Media3 (#24) : l'app tourne sans lecture.
#[cfg(mobile)]
mod unavailable {
    use tokio::sync::mpsc;

    use super::{Engine, Event, Op};
    use crate::i18n::tr;
    use crate::mpv::CacheSize;

    /// Jamais construit : `Launch::start` échoue toujours.
    pub enum Unavailable {}

    impl Engine for Unavailable {
        async fn apply(&self, _op: Op) -> Result<(), String> {
            match *self {}
        }
    }

    pub struct Launch;

    impl Launch {
        pub async fn start(
            &self,
            _cache: CacheSize,
            _audio_device: Option<&str>,
            _events: mpsc::Sender<Event>,
        ) -> Result<Unavailable, String> {
            Err(tr(
                "lecture pas encore disponible sur Android",
                "playback is not available on Android yet",
            )
            .into())
        }
    }
}
