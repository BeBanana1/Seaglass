//! Acteur lecteur : seule tâche qui possède la file, `PlayerState` et le moteur de lecture.
//! La gestion de la fenêtre du moteur (gapless, erreurs) est dans `playback.rs`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot};

use super::listen::ListenTracker;
use super::normalize::Normalizer;
use super::output::{AudioDevices, Output};
use super::queue::{Queue, QueuePart, Repeat};
use super::radio::AutoRadio;
use super::settings::{HiresUpgrade, PlaybackSettings};
use super::sleep::SleepRequest;
use super::source::PlaySource;
use super::state::{PlayerState, QueueView};
use crate::engine::{self, ActiveEngine, Event, Launch};
use crate::i18n::trf;
use crate::integrations::Observers;
use crate::mpv::{MpvIssue, serve::ManifestServer};
use crate::store::Store;
use crate::tidal::TidalClient;
use crate::tidal::models::{AudioQuality, RadioSeed, Stream, StreamFormat, Track, TrackId};

/// Intervalle minimal entre deux événements qui ne changent que `time_pos` (~4 Hz).
const TIME_EMIT_INTERVAL: Duration = Duration::from_millis(250);
/// Nombre de titres du contexte envoyés à l'UI dans `queue-state`.
const UPCOMING_LIMIT: usize = 50;

pub enum Cmd {
    PlayTracks {
        ids: Vec<TrackId>,
        start: usize,
        /// Active la lecture aléatoire et part d'un titre au hasard (`start` est ignoré).
        shuffle: bool,
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
}

/// Destination d'un flux résolu dans la fenêtre du moteur.
#[derive(Debug, Clone, Copy)]
pub enum Slot {
    /// Entrée 0, qui remplace la fenêtre, à partir de `start` secondes.
    Current { start: f64 },
    /// Entrée 1, préchargée à la suite pour le gapless.
    Next(TrackId),
}

#[derive(Clone, Serialize)]
struct Toast {
    level: &'static str,
    message: String,
}

pub(super) struct Player {
    pub(super) app: AppHandle,
    pub(super) tidal: Arc<TidalClient>,
    pub(super) store: Option<Arc<Store>>,
    pub(super) tx: mpsc::Sender<Cmd>,
    pub(super) engine: Result<ActiveEngine, String>,
    /// De quoi relancer le moteur après un arrêt inattendu (#255, `recover.rs`).
    pub(super) launch: Launch,
    pub(super) events_tx: mpsc::Sender<Event>,
    pub(super) last_restart: Option<Instant>,
    pub(super) server: Option<ManifestServer>,
    pub(super) rng: StdRng,
    pub(super) queue: Queue,
    /// File d'avant le dernier vidage ou retrait, tant que rien d'autre ne l'a modifiée
    /// (#349).
    undo_queue: Option<Queue>,
    pub(super) state: PlayerState,
    emitted: PlayerState,
    last_emit: Instant,
    /// Compteurs qui invalident les résolutions devenues obsolètes.
    pub(super) load_req: u64,
    pub(super) preload_req: u64,
    /// Le moteur a le titre courant en entrée 0 (faux après restauration, avant lecture).
    pub(super) loaded: bool,
    pub(super) preloading: bool,
    /// Titre préchargé en entrée 1 du moteur.
    pub(super) next_loaded: Option<(Track, StreamFormat)>,
    /// Titre pour lequel une nouvelle URL a déjà été demandée après une erreur.
    pub(super) retried: Option<TrackId>,
    pub(super) failures: u32,
    /// Qualité demandée à TIDAL (`playback::requested_quality`, réglage sur Android, #325).
    pub(super) quality: AudioQuality,
    /// Lire une autre édition hi-res d'un titre qui n'en a pas (#18), ou la proposer (#59).
    pub(super) hires_upgrade: HiresUpgrade,
    /// Réponses au bandeau « Demander » par album, pour la session : oui = les titres de
    /// l'album passent à l'édition hi-res sans redemander, non = plus rien n'est proposé.
    pub(super) hires_answers: HashMap<u64, bool>,
    /// Invalide les recherches d'offre lancées pour un titre qui n'est plus le courant.
    pub(super) offer_req: u64,
    /// Normalisation du volume (#82) : réglage et ReplayGain des titres chargés.
    pub(super) normalizer: Normalizer,
    /// Périphérique de sortie choisi, liste de mpv et repli (#79).
    pub(super) output: Output,
    /// Pause quand le périphérique utilisé disparaît (#348).
    pub(super) pause_on_unplug: bool,
    /// Échéance du minuteur de mise en veille en durée (#85, état dans `state.sleep`).
    pub(super) sleep_deadline: Option<Instant>,
    /// Radio automatique en fin de file (#78).
    pub(super) radio: AutoRadio,
    pub(super) queue_dirty: bool,
    pub(super) persist_queue: bool,
    pub(super) persist_position: bool,
    pub(super) persist_volume: bool,
    pub(super) last_persist: Instant,
    /// Dernière écriture lancée : une sauvegarde de fermeture l'attend pour ne pas être
    /// écrasée par des données plus anciennes (#260).
    pub(super) last_write: Option<tokio::task::JoinHandle<()>>,
    /// Temps réellement écouté du titre courant, pour l'historique local (#202).
    pub(super) listen: ListenTracker,
    observers: Observers,
}

pub fn spawn(
    app: AppHandle,
    tidal: Arc<TidalClient>,
    store: Option<Arc<Store>>,
    launch: Launch,
    // Lancement par l'utilisateur, sans plantage juste avant : la reprise est permise (#353).
    may_resume: bool,
) -> mpsc::Sender<Cmd> {
    let (tx, rx) = mpsc::channel(64);
    let actor_tx = tx.clone();
    tauri::async_runtime::spawn(async move {
        let (events_tx, events_rx) = mpsc::channel(256);
        let settings = PlaybackSettings::load(store.as_deref());
        let output = Output::new(settings.audio_device.clone());
        let exclusive = super::exclusive::supported() && settings.exclusive;
        let mut engine = launch
            .start(
                settings.cache_size,
                output.launch_device(),
                exclusive,
                events_tx.clone(),
            )
            .await;
        let mut mpv_issue = None;
        if let Err(e) = &engine {
            tracing::error!("{e}");
            mpv_issue = engine::diagnose();
            if mpv_issue == Some(MpvIssue::CpuUnsupported) {
                tracing::error!("processeur sans AVX2 : le mpv embarqué ne peut pas tourner");
                engine = Err(MpvIssue::CpuUnsupported.message().into());
            }
        }
        // Android passe le MPD à Media3 en URL `data:` : pas de serveur local.
        #[cfg(desktop)]
        let server = ManifestServer::start()
            .await
            .inspect_err(|e| tracing::error!("{e}"))
            .ok();
        #[cfg(mobile)]
        let server: Option<ManifestServer> = None;
        let mut player = Player {
            app,
            tidal,
            store,
            tx: actor_tx,
            engine,
            launch,
            events_tx,
            last_restart: None,
            server,
            rng: StdRng::from_os_rng(),
            queue: Queue::default(),
            undo_queue: None,
            state: PlayerState {
                idle: true,
                exclusive,
                mpv_issue,
                ..PlayerState::default()
            },
            emitted: PlayerState::default(),
            last_emit: Instant::now(),
            load_req: 0,
            preload_req: 0,
            loaded: false,
            preloading: false,
            next_loaded: None,
            retried: None,
            failures: 0,
            quality: super::playback::requested_quality(&settings),
            hires_upgrade: settings.hires_upgrade,
            hires_answers: HashMap::new(),
            offer_req: 0,
            normalizer: Normalizer::new(settings.normalization, exclusive),
            output,
            pause_on_unplug: settings.pause_on_unplug,
            sleep_deadline: None,
            radio: AutoRadio::new(settings.auto_radio),
            queue_dirty: true,
            persist_queue: false,
            persist_position: false,
            persist_volume: false,
            last_persist: Instant::now(),
            last_write: None,
            listen: ListenTracker::default(),
            observers: Observers::default(),
        };
        player.restore();
        if may_resume && settings.resume_on_launch {
            player.resume_when_signed_in();
        }
        player.apply_loop_file().await;
        player.apply_volume().await;
        player.run(rx, events_rx).await;
    });
    tx
}

impl Player {
    async fn run(mut self, mut cmds: mpsc::Receiver<Cmd>, mut events: mpsc::Receiver<Event>) {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                cmd = cmds.recv() => match cmd {
                    Some(cmd) => self.handle_cmd(cmd).await,
                    None => break,
                },
                Some(ev) = events.recv() => self.handle_event(ev).await,
                _ = tick.tick() => {
                    self.persist_if_due();
                    self.check_sleep_timer().await;
                }
            }
            self.emit();
        }
    }

    async fn handle_cmd(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::PlayTracks {
                ids,
                start,
                shuffle,
                source,
            } => {
                let start = if shuffle && !ids.is_empty() {
                    self.queue.set_shuffle(true, &mut self.rng);
                    self.rng.random_range(0..ids.len())
                } else {
                    start
                };
                let started = self.queue.set_context(ids, start, source, &mut self.rng);
                if let Some(id) = started {
                    self.on_queue_changed().await;
                    self.load_current(id, 0.0);
                }
            }
            Cmd::PlayNext(id) => {
                self.queue.play_next(id);
                self.on_queue_changed().await;
            }
            Cmd::AddToQueue(id) => {
                self.queue.add_to_queue(id);
                self.on_queue_changed().await;
            }
            Cmd::Enqueue { ids, next } => {
                if !ids.is_empty() {
                    self.queue.enqueue(&ids, next);
                    self.on_queue_changed().await;
                }
            }
            Cmd::RemoveFromQueue(index) => {
                let before = self.queue.clone();
                if self.queue.remove_manual(index) {
                    self.on_queue_changed().await;
                    self.undo_queue = Some(before);
                }
            }
            Cmd::RemoveFromQueueIds(ids) => {
                if self.queue.remove_manual_ids(&ids) > 0 {
                    self.on_queue_changed().await;
                }
            }
            Cmd::MoveInQueue { from, to } => {
                // Le préchargé est toujours `peek_next` : réordonner le milieu de la file ne
                // le retire pas de mpv (gapless intact, pas de nouvelle résolution).
                let next = self.queue.peek_next();
                if self.queue.move_manual(from, to) {
                    if self.queue.peek_next() == next {
                        self.mark_queue_dirty();
                    } else {
                        self.on_queue_changed().await;
                    }
                }
            }
            Cmd::ClearQueue => {
                let before = self.queue.clone();
                if self.queue.clear_upcoming() {
                    self.on_queue_changed().await;
                    self.undo_queue = Some(before);
                }
            }
            Cmd::RestoreQueue(reply) => {
                let restored = self.restore_queue().await;
                let _ = reply.send(restored);
            }
            Cmd::PlayFromQueue { part, index, id } => match self.queue.jump_to(part, index, id) {
                Some(id) => {
                    self.on_queue_changed().await;
                    self.load_current(id, 0.0);
                }
                None => tracing::debug!("file modifiée depuis l'affichage : titre {} ignoré", id.0),
            },
            Cmd::Next => self.skip_next().await,
            Cmd::Previous => self.previous().await,
            Cmd::TogglePause => self.toggle_pause().await,
            Cmd::SetPaused(paused) => self.set_paused(paused).await,
            Cmd::ResumeOnLaunch => self.resume_on_launch(),
            Cmd::AttachObservers(observers) => {
                observers.notify(&self.state);
                self.observers = *observers;
            }
            Cmd::Seek(secs) => self.seek(secs).await,
            Cmd::SetShuffle(on) => {
                self.queue.set_shuffle(on, &mut self.rng);
                self.on_queue_changed().await;
            }
            Cmd::SetRepeat(repeat) => {
                self.queue.set_repeat(repeat);
                self.apply_loop_file().await;
                self.on_queue_changed().await;
            }
            Cmd::SetVolume(level) => {
                self.state.volume.set_level(level);
                self.on_volume_changed().await;
            }
            Cmd::SetMuted(muted) => {
                self.state.volume.set_muted(muted);
                self.on_volume_changed().await;
            }
            Cmd::SetPlaybackSettings(settings) => {
                // Pris en compte à la prochaine résolution : le titre en cours n'est pas
                // rechargé, mais passer à « Demander » propose déjà son édition hi-res.
                if settings.hires_upgrade != self.hires_upgrade {
                    self.hires_upgrade = settings.hires_upgrade;
                    self.maybe_offer_hires();
                }
                // Le suivant préchargé est dans l'ancienne qualité : il sera redemandé.
                let quality = super::playback::requested_quality(&settings);
                if quality != self.quality {
                    self.quality = quality;
                    self.invalidate_next().await;
                }
                self.radio.enabled = settings.auto_radio;
                self.pause_on_unplug = settings.pause_on_unplug;
                self.apply_cache_size(settings.cache_size).await;
                self.set_normalization(settings.normalization).await;
                self.set_audio_device(settings.audio_device).await;
                self.set_exclusive(settings.exclusive).await;
            }
            Cmd::AnswerHiresOffer(accept) => self.answer_hires_offer(accept).await,
            Cmd::SetSleepTimer(request) => self.set_sleep_timer(request).await,
            Cmd::HiresOffer { request, offer } => {
                if request == self.offer_req {
                    self.state.hires_offer = Some(offer);
                }
            }
            Cmd::GetState(reply) => {
                let _ = reply.send(self.state.clone());
            }
            Cmd::Flush(reply) => self.flush(reply),
            Cmd::GetQueue(reply) => {
                let _ = reply.send(self.queue_view());
            }
            Cmd::GetPlayOrder(limit, reply) => {
                let _ = reply.send(self.queue.play_order(limit));
            }
            Cmd::GetAudioDevices(reply) => {
                let _ = reply.send(self.output.devices());
            }
            Cmd::Resolved {
                request,
                slot,
                result,
            } => self.on_resolved(request, slot, result).await,
            Cmd::RadioResolved {
                request,
                seed,
                result,
            } => self.on_radio_resolved(request, seed, result).await,
        }
    }

    async fn handle_event(&mut self, ev: Event) {
        let s = &mut self.state;
        match ev {
            // Position et durée ne valent que pour un titre chargé : avant, elles écraseraient
            // la position restaurée ou celle du titre en cours de résolution.
            Event::Position(t) if self.loaded => {
                s.time_pos = t.unwrap_or(0.0);
                self.persist_position = true;
                self.note_listen();
                self.maybe_preload();
            }
            Event::Duration(d) if self.loaded => s.duration = d.unwrap_or(0.0),
            Event::Position(_) | Event::Duration(_) => {}
            // Sans titre chargé, la pause reste celle de l'état restauré.
            Event::Paused(p) if self.loaded => {
                s.paused = p;
                self.persist_position = true;
            }
            Event::Paused(_) => {}
            Event::Buffering(b) => s.buffering = b,
            Event::Format(a) => s.set_output(a),
            Event::DeviceFormat(a) => s.set_device_format(a),
            Event::Driver(ao) => s.output_driver = ao,
            Event::Idle(i) => s.set_idle(i),
            Event::Advanced => self.on_advanced().await,
            Event::Devices(list) => self.on_device_list(list).await,
            Event::Loaded => self.failures = 0,
            Event::Ended(end) => self.on_end_file(end).await,
            Event::Stopped => self.on_engine_stopped().await,
        }
    }

    /// Toute modification de la file invalide le titre préchargé (fenêtre de 2 entrées).
    pub(super) async fn on_queue_changed(&mut self) {
        self.invalidate_next().await;
        self.mark_queue_dirty();
    }

    /// Remet la file gardée avant un vidage ou un retrait (#349), si le titre en cours est
    /// toujours le même. Toute autre modification de la file l'a déjà oubliée.
    async fn restore_queue(&mut self) -> bool {
        match self.undo_queue.take() {
            Some(before) if before.current() == self.queue.current() => {
                self.queue = before;
                self.on_queue_changed().await;
                true
            }
            _ => false,
        }
    }

    pub(super) fn mark_queue_dirty(&mut self) {
        self.undo_queue = None;
        self.queue_dirty = true;
        self.persist_queue = true;
        self.pin_queue_tracks();
        // File changée : une radio peut de nouveau être demandée.
        self.radio.asked = None;
    }

    /// Protège de l'éviction du cache borné les titres de la file (#90) : `queue_view` et
    /// la sauvegarde les y relisent.
    pub(super) fn pin_queue_tracks(&self) {
        self.tidal.cache.set_pinned(self.queue.referenced_ids());
    }

    fn queue_view(&self) -> QueueView {
        QueueView {
            manual: self.tidal.cache.get_many(self.queue.manual()),
            upcoming: self
                .tidal
                .cache
                .get_many(self.queue.upcoming_context(UPCOMING_LIMIT)),
            radio: self.queue.radio().and_then(|id| self.tidal.cache.get(id)),
            source: self.play_source(),
        }
    }

    /// « Joué depuis » (#124) : la radio automatique si le titre courant en vient, sinon la
    /// source donnée avec le contexte.
    fn play_source(&self) -> Option<PlaySource> {
        match self.queue.radio_playing() {
            Some(seed) => self.tidal.cache.get(seed).map(|track| PlaySource::Radio {
                seed: RadioSeed::Track,
                id: seed.0,
                title: trf!("Radio de {}", "{} radio", track.title),
            }),
            None => self.queue.source().cloned(),
        }
    }

    fn emit(&mut self) {
        let s = &mut self.state;
        s.shuffle = self.queue.shuffle();
        s.repeat = self.queue.repeat();
        s.has_previous = self.queue.has_previous();
        s.has_next = self.queue.peek_next().is_some();

        if self.queue_dirty {
            self.queue_dirty = false;
            if let Err(e) = self.app.emit("queue-state", self.queue_view()) {
                tracing::warn!("émission queue-state : {e}");
            }
        }
        if self.state == self.emitted
            || (self.emitted.differs_only_by_time(&self.state)
                && self.last_emit.elapsed() < TIME_EMIT_INTERVAL)
        {
            return;
        }
        if let Err(e) = self.app.emit("player-state", &self.state) {
            tracing::warn!("émission player-state : {e}");
        }
        self.observers.notify(&self.state);
        self.emitted = self.state.clone();
        self.last_emit = Instant::now();
    }

    pub(super) fn toast(&self, level: &'static str, message: String) {
        tracing::warn!("{message}");
        let _ = self.app.emit("toast", Toast { level, message });
    }
}
