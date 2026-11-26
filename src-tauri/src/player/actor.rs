//! Acteur lecteur : seule tâche qui possède la file, `PlayerState` et le moteur de lecture.
//! La gestion de la fenêtre du moteur (gapless, erreurs) est dans `playback.rs`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

use super::blind::{self, BlindTest};
use super::eq::Equalizer;
use super::listen::ListenTracker;
use super::normalize::Normalizer;
use super::output::Output;
use super::playlog::PlayLogTracker;
use super::profiles::{self, Profiles};
use super::quality::QualityPolicy;
use super::queue::Queue;
use super::radio::AutoRadio;
use super::settings::{HiresUpgrade, PlaybackSettings};
use super::source::PlaySource;
use super::state::{PlayerState, QueueView};
use super::takeover::Takeover;
use crate::engine::{self, ActiveEngine, Event, Launch};
use crate::i18n::trf;
use crate::integrations::Observers;
use crate::mpv::{MpvIssue, serve::ManifestServer};
use crate::store::Store;
use crate::tidal::TidalClient;
use crate::tidal::models::{PlaybackSession, RadioSeed, StreamFormat, Track, TrackId};

/// Intervalle minimal entre deux événements qui ne changent que `time_pos` (~4 Hz).
const TIME_EMIT_INTERVAL: Duration = Duration::from_millis(250);
/// Nombre de titres du contexte envoyés à l'UI dans `queue-state`.
const UPCOMING_LIMIT: usize = 50;

pub use super::command::{Cmd, Slot};

#[derive(Clone, Serialize)]
struct Toast {
    level: &'static str,
    message: String,
    /// Refus du lecteur (clin d'œil #399) : l'UI peut l'habiller selon le thème, le message
    /// reste celui-ci.
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<&'static str>,
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
    pub(super) undo_queue: Option<Queue>,
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
    pub(super) next_loaded: Option<(Track, StreamFormat, PlaybackSession)>,
    /// Titre pour lequel une nouvelle URL a déjà été demandée après une erreur.
    pub(super) retried: Option<TrackId>,
    pub(super) failures: u32,
    /// Règles de la qualité demandée à TIDAL (réglages d'Android, #325, #365, #366).
    pub(super) quality: QualityPolicy,
    /// Lire une autre édition hi-res d'un titre qui n'en a pas (#18), ou la proposer (#59).
    pub(super) hires_upgrade: HiresUpgrade,
    /// Réponses au bandeau « Demander » par album, pour la session : oui = les titres de
    /// l'album passent à l'édition hi-res sans redemander, non = plus rien n'est proposé.
    pub(super) hires_answers: HashMap<u64, bool>,
    /// Invalide les recherches d'offre lancées pour un titre qui n'est plus le courant.
    pub(super) offer_req: u64,
    /// Normalisation du volume (#82) : réglage et ReplayGain des titres chargés.
    pub(super) normalizer: Normalizer,
    /// Égaliseur (#428) : réglage et profil appliqué.
    pub(super) equalizer: Equalizer,
    /// Périphérique de sortie choisi, liste de mpv et repli (#79).
    pub(super) output: Output,
    /// Pause quand le périphérique utilisé disparaît (#348).
    pub(super) pause_on_unplug: bool,
    /// Réglages audio retenus par périphérique de sortie (#416).
    pub(super) profiles: Profiles,
    /// Échéance du minuteur de mise en veille en durée (#85, état dans `state.sleep`).
    pub(super) sleep_deadline: Option<Instant>,
    /// Radio automatique en fin de file (#78).
    pub(super) radio: AutoRadio,
    pub(super) queue_dirty: bool,
    pub(super) persist_queue: bool,
    pub(super) persist_position: bool,
    pub(super) persist_volume: bool,
    pub(super) persist_profiles: bool,
    pub(super) last_persist: Instant,
    /// Dernière écriture lancée : une sauvegarde de fermeture l'attend pour ne pas être
    /// écrasée par des données plus anciennes (#260).
    pub(super) last_write: Option<tokio::task::JoinHandle<()>>,
    /// Temps réellement écouté du titre courant, pour l'historique local (#202).
    pub(super) listen: ListenTracker,
    /// Listen of the current stream for TIDAL's play log (#34).
    pub(super) playlog: PlayLogTracker,
    /// Partie de blind test en cours (#434), avec la file mise de côté.
    pub(super) blind: Option<BlindTest>,
    /// La partie a changé : `blind-test` part à la prochaine émission.
    pub(super) blind_dirty: bool,
    /// One stream per account (#35).
    pub(super) takeover: Takeover,
    pub(super) observers: Observers,
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
        let device_profiles =
            Profiles::new(settings.device_profiles, profiles::load(store.as_deref()));
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
        let takeover = Takeover::start(Arc::clone(&tidal), actor_tx.clone());
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
            quality: QualityPolicy::new(&settings),
            hires_upgrade: settings.hires_upgrade,
            hires_answers: HashMap::new(),
            offer_req: 0,
            normalizer: Normalizer::new(settings.normalization, exclusive),
            equalizer: Equalizer::new(settings.equalizer.clone()),
            output,
            pause_on_unplug: settings.pause_on_unplug,
            profiles: device_profiles,
            sleep_deadline: None,
            radio: AutoRadio::new(settings.auto_radio),
            queue_dirty: true,
            persist_queue: false,
            persist_position: false,
            persist_volume: false,
            persist_profiles: false,
            last_persist: Instant::now(),
            last_write: None,
            listen: ListenTracker::default(),
            playlog: PlayLogTracker::default(),
            blind: None,
            blind_dirty: false,
            takeover,
            observers: Observers::default(),
        };
        player.state.equalizer = player.equalizer.applied_name();
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
                    self.check_blind_deadline().await;
                }
            }
            self.emit();
            self.sync_takeover();
        }
    }

    /// Toute modification de la file invalide le titre préchargé (fenêtre de 2 entrées).
    pub(super) async fn on_queue_changed(&mut self) {
        self.invalidate_next().await;
        self.mark_queue_dirty();
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
        let mut ids = self.queue.referenced_ids();
        if let Some(blind) = &self.blind {
            ids.extend(blind.saved.referenced_ids());
        }
        self.tidal.cache.set_pinned(ids);
    }

    /// File de l'utilisateur, celle mise de côté pendant un blind test (#434).
    pub(super) fn queue_view(&self) -> QueueView {
        let queue = self.main_queue();
        QueueView {
            manual: self.tidal.cache.get_many(queue.manual()),
            upcoming: self
                .tidal
                .cache
                .get_many(queue.upcoming_context(UPCOMING_LIMIT)),
            radio: queue.radio().and_then(|id| self.tidal.cache.get(id)),
            source: self.play_source(),
        }
    }

    /// « Joué depuis » (#124) : la radio automatique si le titre courant en vient, sinon la
    /// source donnée avec le contexte.
    fn play_source(&self) -> Option<PlaySource> {
        let queue = self.main_queue();
        match queue.radio_playing() {
            Some(seed) => self.tidal.cache.get(seed).map(|track| PlaySource::Radio {
                seed: RadioSeed::Track,
                id: seed.0,
                title: trf!("Radio de {}", "{} radio", track.title),
            }),
            None => queue.source().cloned(),
        }
    }

    fn emit(&mut self) {
        let playing = self.blind.is_some();
        let queue = self.main_queue();
        let (shuffle, repeat) = (queue.shuffle(), queue.repeat());
        let s = &mut self.state;
        s.shuffle = shuffle;
        s.repeat = repeat;
        // Blind test : Précédent réécoute l'extrait, Suivant passe la manche.
        s.has_previous = playing || self.queue.has_previous();
        s.has_next = playing || self.queue.peek_next().is_some();
        s.blind_test = playing;
        s.next_ready = self.next_loaded.is_some();
        s.soundtrack = s
            .track
            .as_ref()
            .is_some_and(|t| crate::tidal::soundtracks::is_soundtrack(t.album.id));

        if std::mem::take(&mut self.blind_dirty) {
            self.emit_blind_test();
        }
        if self.queue_dirty {
            self.queue_dirty = false;
            if let Err(e) = self.app.emit("queue-state", self.queue_view()) {
                tracing::warn!("émission queue-state : {e}");
            }
        }
        // Sans le titre tant qu'une manche de blind test n'est pas révélée (#434).
        let state = blind::public_state(&self.state, self.masks_track());
        if *state == self.emitted
            || (self.emitted.differs_only_by_time(&state)
                && self.last_emit.elapsed() < TIME_EMIT_INTERVAL)
        {
            return;
        }
        if let Err(e) = self.app.emit("player-state", &*state) {
            tracing::warn!("émission player-state : {e}");
        }
        self.observers.notify(&state);
        self.emitted = state.into_owned();
        self.last_emit = Instant::now();
    }

    pub(super) fn toast(&self, level: &'static str, message: String) {
        self.toast_kind(level, None, message);
    }

    /// Toast d'un refus nommé (`unplayable`, `stopped`), voir `Toast::kind`.
    pub(super) fn toast_kind(
        &self,
        level: &'static str,
        kind: Option<&'static str>,
        message: String,
    ) {
        tracing::warn!("{message}");
        let _ = self.app.emit(
            "toast",
            Toast {
                level,
                message,
                kind,
            },
        );
    }
}
