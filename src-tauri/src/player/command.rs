//! Commands of the player actor (`Cmd`) and destination of a resolved stream (`Slot`).

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
        /// Album, playlist… the context comes from ("Playing from", #124).
        source: Option<PlaySource>,
    },
    PlayNext(TrackId),
    AddToQueue(TrackId),
    /// Several tracks in the given order: at the head of the manual queue if `next`, else at
    /// the end.
    Enqueue {
        ids: Vec<TrackId>,
        next: bool,
    },
    RemoveFromQueue(usize),
    /// Removes entries from the manual queue by id (Jam guests, #223).
    RemoveFromQueueIds(Vec<TrackId>),
    /// Moves an entry of the manual queue (`from` ends up at index `to`).
    MoveInQueue {
        from: usize,
        to: usize,
    },
    /// Clears the manual queue and the rest of the context; the current track goes on.
    ClearQueue,
    /// Undoes the last "Clear queue" or track removal (#349); answers `false` if the queue
    /// changed since.
    RestoreQueue(oneshot::Sender<bool>),
    /// Plays a queue track as the UI shows it (#167); ignored if it is no longer `id`.
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
    /// Level 0–100, already checked by the command.
    SetVolume(u8),
    /// One volume step from the level heard (global shortcuts, #413, desktop only).
    #[cfg(desktop)]
    StepVolume(i8),
    SetMuted(bool),
    /// Saved playback settings, applied live (mpv cache size, hi-res edition).
    SetPlaybackSettings(PlaybackSettings),
    /// Answer to the banner offering the hi-res edition of the current track (#59).
    AnswerHiresOffer(bool),
    /// Sleep timer (#85), already checked; `None` cancels it.
    SetSleepTimer(Option<SleepRequest>),
    GetState(oneshot::Sender<PlayerState>),
    /// Saves the queue, position and volume at once, on close (#260); the answer goes out
    /// once the write is done. Synchronous channel: awaited from the main thread
    /// (`RunEvent::Exit`) or the updater, outside tokio.
    Flush(std::sync::mpsc::Sender<()>),
    GetQueue(oneshot::Sender<QueueView>),
    /// Handoff to another device (#414): snapshot of the playback, then pause; `None` if
    /// nothing is playing.
    TakeHandoff(oneshot::Sender<Option<Handoff>>),
    /// Resumes a handed-off playback, already checked (`Handoff::received`); answers `false`
    /// if it has no current track.
    ApplyHandoff(Box<Handoff>, oneshot::Sender<bool>),
    /// Full play order, capped (#327).
    GetPlayOrder(usize, oneshot::Sender<Vec<TrackId>>),
    /// Output devices known to mpv, for Settings (#79).
    GetAudioDevices(oneshot::Sender<AudioDevices>),
    /// Explicit play (`false`) or pause (`true`): media keys (SMTC, Android media session)
    /// and Jam guests (#222).
    SetPaused(bool),
    /// Session resumed at launch: plays the restored track if nothing was started in the
    /// meantime (#353).
    ResumeOnLaunch,
    /// Attaches the system integrations (SMTC…), created after the actor.
    /// Boxed: the integration handles would make every command heavier.
    AttachObservers(Box<Observers>),
    /// Blind test (#434).
    BlindTest(BlindCmd),
    /// Result of a network resolution (metadata + stream) started by the actor.
    Resolved {
        request: u64,
        slot: Slot,
        result: Result<Box<(Track, Stream)>, String>,
    },
    /// Hi-res edition found for the current track, to offer ("Ask" mode).
    HiresOffer {
        request: u64,
        offer: StreamFormat,
    },
    /// Radio of track `seed`, asked for when the queue runs out (#78).
    RadioResolved {
        request: u64,
        seed: TrackId,
        result: Result<Vec<TrackId>, String>,
    },
    /// Another client of the account started playing (#35, `takeover.rs`).
    PlaybackTaken(PlaybackTaken),
}

/// Where a resolved stream goes in the engine window.
#[derive(Debug, Clone, Copy)]
pub enum Slot {
    /// Entry 0, which replaces the window, from `start` seconds.
    Current { start: f64 },
    /// Entry 1, preloaded after it for gapless.
    Next(TrackId),
}
