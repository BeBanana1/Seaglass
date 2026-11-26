//! Commandes de l'acteur lecteur (`Cmd`) et destination d'un flux résolu (`Slot`).

use tokio::sync::oneshot;

use super::blind::BlindCmd;
use super::handoff::Handoff;
use super::output::AudioDevices;
use super::queue::{QueuePart, Repeat};
use super::settings::PlaybackSettings;
use super::sleep::SleepRequest;
use super::source::PlaySource;
use super::state::{PlayerState, QueueView};
use crate::integrations::Observers;
use crate::tidal::models::{PlaybackTaken, Stream, StreamFormat, Track, TrackId};

pub enum Cmd {
    PlayTracks {
        ids: Vec<TrackId>,
        start: usize,
        /// `Some(true)`: turns shuffle on and starts on a random track (`start` is ignored).
        /// `Some(false)`: turns shuffle off, as the "Play" buttons do (#526). `None`: keeps
        /// the current mode (a track picked in a list).
        shuffle: Option<bool>,
        /// Album, playlist… d'où vient le contexte (« Joué depuis », #124).
        source: Option<PlaySource>,
    },
    PlayNext(TrackId),
    AddToQueue(TrackId),
    /// Plusieurs titres dans l'ordre donné : en tête de la file manuelle si `next`, sinon à la fin.
    Enqueue {
        ids: Vec<TrackId>,
        next: bool,
    },
    RemoveFromQueue(usize),
    /// Retire de la file manuelle une entrée par identifiant (invités de la Jam, #223).
    RemoveFromQueueIds(Vec<TrackId>),
    /// Déplace une entrée de la file manuelle (`from` finit à l'index `to`).
    MoveInQueue {
        from: usize,
        to: usize,
    },
    /// Vide la file manuelle et la suite du contexte ; le titre courant continue.
    ClearQueue,
    /// Annule le dernier « Vider la file » ou retrait d'un titre (#349) ; répond `false`
    /// si la file a changé depuis.
    RestoreQueue(oneshot::Sender<bool>),
    /// Lit un titre de la file tel que l'UI l'affiche (#167) ; ignoré si ce n'est plus `id`.
    PlayFromQueue {
        part: QueuePart,
        index: usize,
        id: TrackId,
    },
    Next,
    Previous,
    TogglePause,
    Seek(f64),
    SetShuffle(bool),
    SetRepeat(Repeat),
    /// Niveau 0–100, déjà validé par la commande.
    SetVolume(u8),
    /// Un cran de volume relatif au niveau entendu (raccourcis globaux, #413, bureau seulement).
    #[cfg(desktop)]
    StepVolume(i8),
    SetMuted(bool),
    /// Réglages de lecture enregistrés, appliqués à chaud (taille du cache mpv, édition
    /// hi-res).
    SetPlaybackSettings(PlaybackSettings),
    /// Réponse au bandeau qui propose l'édition hi-res du titre courant (#59).
    AnswerHiresOffer(bool),
    /// Minuteur de mise en veille (#85), déjà validé ; `None` l'annule.
    SetSleepTimer(Option<SleepRequest>),
    GetState(oneshot::Sender<PlayerState>),
    /// Sauvegarde immédiate de la file, de la position et du volume, à la fermeture
    /// (#260) ; la réponse part une fois l'écriture terminée. Canal synchrone : attendu
    /// depuis le thread principal (`RunEvent::Exit`) ou l'updater, hors de tokio.
    Flush(std::sync::mpsc::Sender<()>),
    GetQueue(oneshot::Sender<QueueView>),
    /// Transfert vers un autre appareil (#414) : instantané de la lecture, puis pause ;
    /// `None` si rien n'est en cours.
    TakeHandoff(oneshot::Sender<Option<Handoff>>),
    /// Reprend une lecture transférée, déjà vérifiée (`Handoff::received`) ; répond
    /// `false` si elle n'a pas de titre courant.
    ApplyHandoff(Box<Handoff>, oneshot::Sender<bool>),
    /// Ordre de lecture complet, plafonné (#327).
    GetPlayOrder(usize, oneshot::Sender<Vec<TrackId>>),
    /// Périphériques de sortie connus de mpv, pour les Réglages (#79).
    GetAudioDevices(oneshot::Sender<AudioDevices>),
    /// Lecture (`false`) ou pause (`true`) explicite : touches média (SMTC, session média
    /// d'Android) et invités de la Jam (#222).
    SetPaused(bool),
    /// Session reprise au lancement : lire le titre restauré si rien n'a été lancé
    /// entre-temps (#353).
    ResumeOnLaunch,
    /// Branche les intégrations système (SMTC…), créées après l'acteur.
    /// En boîte : les poignées des intégrations alourdiraient toutes les commandes.
    AttachObservers(Box<Observers>),
    /// Blind test (#434).
    BlindTest(BlindCmd),
    /// Résultat d'une résolution réseau (métadonnées + flux) lancée par l'acteur.
    Resolved {
        request: u64,
        slot: Slot,
        result: Result<Box<(Track, Stream)>, String>,
    },
    /// Édition hi-res trouvée pour le titre courant, à proposer (mode « Demander »).
    HiresOffer {
        request: u64,
        offer: StreamFormat,
    },
    /// Radio du titre `seed`, demandée quand la file s'épuise (#78).
    RadioResolved {
        request: u64,
        seed: TrackId,
        result: Result<Vec<TrackId>, String>,
    },
    /// Another client of the account started playing (#35, `takeover.rs`).
    PlaybackTaken(PlaybackTaken),
}

/// Destination d'un flux résolu dans la fenêtre du moteur.
#[derive(Debug, Clone, Copy)]
pub enum Slot {
    /// Entrée 0, qui remplace la fenêtre, à partir de `start` secondes.
    Current { start: f64 },
    /// Entrée 1, préchargée à la suite pour le gapless.
    Next(TrackId),
}
