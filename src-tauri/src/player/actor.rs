//! Player actor: the only task that owns the queue, `PlayerState` and the playback engine.
//! Commands are in `command.rs` and routed by `dispatch.rs`; the engine window (gapless,
//! errors) is handled in `playback.rs`.

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

/// Minimum interval between two events that only change `time_pos` (~4 Hz).
const TIME_EMIT_INTERVAL: Duration = Duration::from_millis(250);
/// Number of context tracks sent to the UI in `queue-state`.
const UPCOMING_LIMIT: usize = 50;

pub use super::command::{Cmd, Slot};

#[derive(Clone, Serialize)]
struct Toast {
    level: &'static str,
    message: String,
    /// Refusal by the player (easter egg #399): the UI may dress it up per theme, the
    /// message stays this one.
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<&'static str>,
}

pub(super) struct Player {
    pub(super) app: AppHandle,
    pub(super) tidal: Arc<TidalClient>,
    pub(super) store: Option<Arc<Store>>,
    pub(super) tx: mpsc::Sender<Cmd>,
    pub(super) engine: Result<ActiveEngine, String>,
    /// What it takes to restart the engine after an unexpected stop (#255, `recover.rs`).
    pub(super) launch: Launch,
    pub(super) events_tx: mpsc::Sender<Event>,
    pub(super) last_restart: Option<Instant>,
    pub(super) server: Option<ManifestServer>,
    pub(super) rng: StdRng,
    pub(super) queue: Queue,
    /// Queue from before the last clear or removal, as long as nothing else changed it
    /// (#349).
    pub(super) undo_queue: Option<Queue>,
    pub(super) state: PlayerState,
    emitted: PlayerState,
    last_emit: Instant,
    /// Counters that invalidate resolutions gone stale.
    pub(super) load_req: u64,
    pub(super) preload_req: u64,
    /// The engine has the current track as entry 0 (false after a restore, before playing).
    pub(super) loaded: bool,
    pub(super) preloading: bool,
    /// Track preloaded as entry 1 of the engine.
    pub(super) next_loaded: Option<(Track, StreamFormat, PlaybackSession)>,
    /// Track for which a new URL was already requested after an error.
    pub(super) retried: Option<TrackId>,
    pub(super) failures: u32,
    /// Rules for the quality requested from TIDAL (Android settings, #325, #365, #366).
    pub(super) quality: QualityPolicy,
    /// Play another hi-res edition of a track that is not hi-res (#18), or offer it (#59).
    pub(super) hires_upgrade: HiresUpgrade,
    /// Answers to the "Ask" banner per album, for the session: yes = the album's tracks move
    /// to the hi-res edition without asking again, no = nothing more is offered.
    pub(super) hires_answers: HashMap<u64, bool>,
    /// Invalidates offer lookups started for a track that is no longer the current one.
    pub(super) offer_req: u64,
    /// Volume normalization (#82): setting and ReplayGain of the loaded tracks.
    pub(super) normalizer: Normalizer,
    /// Equalizer (#428): setting and applied profile.
    pub(super) equalizer: Equalizer,
    /// Chosen output device, mpv's list and fallback (#79).
    pub(super) output: Output,
    /// Pause when the device in use goes away (#348).
    pub(super) pause_on_unplug: bool,
    /// Audio settings kept per output device (#416).
    pub(super) profiles: Profiles,
    /// Deadline of a duration sleep timer (#85, state in `state.sleep`).
    pub(super) sleep_deadline: Option<Instant>,
    /// Automatic radio at the end of the queue (#78).
    pub(super) radio: AutoRadio,
    pub(super) queue_dirty: bool,
    pub(super) persist_queue: bool,
    pub(super) persist_position: bool,
    pub(super) persist_volume: bool,
    pub(super) persist_profiles: bool,
    pub(super) last_persist: Instant,
    /// Last write started: a save on close waits for it, so older data does not overwrite
    /// it (#260).
    pub(super) last_write: Option<tokio::task::JoinHandle<()>>,
    /// Time actually listened to the current track, for the local history (#202).
    pub(super) listen: ListenTracker,
    /// Listen of the current stream for TIDAL's play log (#34).
    pub(super) playlog: PlayLogTracker,
    /// Blind test game in progress (#434), with the queue set aside.
    pub(super) blind: Option<BlindTest>,
    /// The game changed: `blind-test` goes out with the next emit.
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
    // Launched by the user, with no crash just before: resuming is allowed (#353).
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
                tracing::error!("CPU without AVX2: the bundled mpv cannot run");
                engine = Err(MpvIssue::CpuUnsupported.message().into());
            }
        }
        // Android hands the MPD to Media3 as a `data:` URL: no local server.
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

    /// Any change to the queue invalidates the preloaded track (window of 2 entries).
    pub(super) async fn on_queue_changed(&mut self) {
        self.invalidate_next().await;
        self.mark_queue_dirty();
    }

    pub(super) fn mark_queue_dirty(&mut self) {
        self.undo_queue = None;
        self.queue_dirty = true;
        self.persist_queue = true;
        self.pin_queue_tracks();
        // Queue changed: a radio may be asked for again.
        self.radio.asked = None;
    }

    /// Shields the queue's tracks from eviction from the bounded cache (#90): `queue_view`
    /// and the save read them back from it.
    pub(super) fn pin_queue_tracks(&self) {
        let mut ids = self.queue.referenced_ids();
        if let Some(blind) = &self.blind {
            ids.extend(blind.saved.referenced_ids());
        }
        self.tidal.cache.set_pinned(ids);
    }

    /// The user's queue, the one set aside during a blind test (#434).
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

    /// "Playing from" (#124): the automatic radio if the current track comes from it, else
    /// the source given with the context.
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
        // Blind test: Previous replays the clip, Next skips the round.
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
                tracing::warn!("emitting queue-state: {e}");
            }
        }
        // Without the track while a blind test round is not revealed (#434).
        let state = blind::public_state(&self.state, self.masks_track());
        if *state == self.emitted
            || (self.emitted.differs_only_by_time(&state)
                && self.last_emit.elapsed() < TIME_EMIT_INTERVAL)
        {
            return;
        }
        if let Err(e) = self.app.emit("player-state", &*state) {
            tracing::warn!("emitting player-state: {e}");
        }
        self.observers.notify(&state);
        self.emitted = state.into_owned();
        self.last_emit = Instant::now();
    }

    pub(super) fn toast(&self, level: &'static str, message: String) {
        self.toast_kind(level, None, message);
    }

    /// Toast of a named refusal (`unplayable`, `stopped`), see `Toast::kind`.
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
