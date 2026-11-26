//! System Media Transport Controls: keyboard media keys, the Windows media flyout and the lock
//! screen.
//!
//! The WinRT objects live on a dedicated thread (MTA); the actor sends it `NowPlaying`s and the
//! thread only touches SMTC when something changed.

use std::sync::mpsc as std_mpsc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use windows::Foundation::{TimeSpan, TypedEventHandler, Uri};
use windows::Media::{
    MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls,
    SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs,
    SystemMediaTransportControlsTimelineProperties,
};
use windows::Storage::Streams::RandomAccessStreamReference;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::WinRT::{
    ISystemMediaTransportControlsInterop, RO_INIT_MULTITHREADED, RoInitialize,
};
use windows::core::{HSTRING, Ref, factory};

use super::{MediaInfo, NowPlaying, PlaybackStatus};
use crate::player::Cmd;

/// Periodic update of the position Windows shows during playback.
const TIMELINE_INTERVAL: Duration = Duration::from_secs(5);
/// Tolerated gap before treating a change as a jump (seek).
const SEEK_TOLERANCE: f64 = 1.5;

pub struct SmtcHandle {
    tx: std_mpsc::Sender<NowPlaying>,
}

impl SmtcHandle {
    pub fn update(&self, state: NowPlaying) {
        let _ = self.tx.send(state);
    }
}

/// Attaches SMTC to the window `hwnd` (raw handle value).
pub fn start(hwnd: isize, player: mpsc::Sender<Cmd>) -> Result<SmtcHandle, String> {
    let (tx, rx) = std_mpsc::channel::<NowPlaying>();
    let (ready_tx, ready_rx) = std_mpsc::channel();
    std::thread::Builder::new()
        .name("smtc".into())
        .spawn(move || match Session::new(hwnd, player) {
            Ok(mut session) => {
                let _ = ready_tx.send(Ok(()));
                for state in rx {
                    if let Err(e) = session.apply(&state) {
                        tracing::warn!("SMTC : {e}");
                    }
                }
            }
            Err(e) => {
                let _ = ready_tx.send(Err(e.to_string()));
            }
        })
        .map_err(|e| e.to_string())?;
    ready_rx
        .recv()
        .map_err(|_| "SMTC thread stopped".to_owned())??;
    Ok(SmtcHandle { tx })
}

struct Session {
    smtc: SystemMediaTransportControls,
    last: Option<NowPlaying>,
    timeline_at: Option<(Instant, f64)>,
}

impl Session {
    fn new(hwnd: isize, player: mpsc::Sender<Cmd>) -> windows::core::Result<Self> {
        // Already initialized or a different mode: no consequence for our calls.
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
        let interop =
            factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>()?;
        let smtc: SystemMediaTransportControls =
            unsafe { interop.GetForWindow(HWND(hwnd as *mut _)) }?;
        smtc.SetIsEnabled(true)?;
        smtc.SetIsPlayEnabled(true)?;
        smtc.SetIsPauseEnabled(true)?;
        smtc.SetIsStopEnabled(false)?;
        smtc.DisplayUpdater()?.SetType(MediaPlaybackType::Music)?;
        smtc.ButtonPressed(&TypedEventHandler::new(
            move |_: Ref<SystemMediaTransportControls>,
                  args: Ref<SystemMediaTransportControlsButtonPressedEventArgs>| {
                let cmd = match args.ok()?.Button()? {
                    SystemMediaTransportControlsButton::Play => Some(Cmd::SetPaused(false)),
                    SystemMediaTransportControlsButton::Pause => Some(Cmd::SetPaused(true)),
                    SystemMediaTransportControlsButton::Next => Some(Cmd::Next),
                    SystemMediaTransportControlsButton::Previous => Some(Cmd::Previous),
                    _ => None,
                };
                if let Some(cmd) = cmd {
                    // WinRT thread: no waiting allowed, the actor's queue is enough.
                    let _ = player.try_send(cmd);
                }
                Ok(())
            },
        ))?;
        Ok(Self {
            smtc,
            last: None,
            timeline_at: None,
        })
    }

    fn apply(&mut self, s: &NowPlaying) -> windows::core::Result<()> {
        let last = self.last.as_ref();
        let media_changed = last.map(|l| &l.media) != Some(&s.media);
        let status_changed = last.map(|l| l.status) != Some(s.status);

        if media_changed {
            self.set_media(s.media.as_ref())?;
        }
        if status_changed {
            self.smtc.SetPlaybackStatus(match s.status {
                PlaybackStatus::Stopped => MediaPlaybackStatus::Stopped,
                PlaybackStatus::Changing => MediaPlaybackStatus::Changing,
                PlaybackStatus::Playing => MediaPlaybackStatus::Playing,
                PlaybackStatus::Paused => MediaPlaybackStatus::Paused,
            })?;
        }
        if last.map(|l| (l.has_next, l.has_previous)) != Some((s.has_next, s.has_previous)) {
            self.smtc.SetIsNextEnabled(s.has_next)?;
            self.smtc.SetIsPreviousEnabled(s.has_previous)?;
        }
        if media_changed || status_changed || self.timeline_due(s) {
            self.set_timeline(s)?;
        }
        self.last = Some(s.clone());
        Ok(())
    }

    fn set_media(&self, media: Option<&MediaInfo>) -> windows::core::Result<()> {
        let updater = self.smtc.DisplayUpdater()?;
        let Some(m) = media else {
            return updater.ClearAll();
        };
        updater.SetType(MediaPlaybackType::Music)?;
        let music = updater.MusicProperties()?;
        music.SetTitle(&HSTRING::from(&m.title))?;
        music.SetArtist(&HSTRING::from(&m.artist))?;
        music.SetAlbumTitle(&HSTRING::from(&m.album))?;
        match &m.cover_url {
            Some(url) => {
                let uri = Uri::CreateUri(&HSTRING::from(url))?;
                updater.SetThumbnail(&RandomAccessStreamReference::CreateFromUri(&uri)?)?;
            }
            None => updater.SetThumbnail(None)?,
        }
        updater.Update()
    }

    /// Position to refresh: every 5 s during playback, or after a jump.
    fn timeline_due(&self, s: &NowPlaying) -> bool {
        let Some((at, pos)) = self.timeline_at else {
            return true;
        };
        let elapsed = at.elapsed();
        let expected = if s.status == PlaybackStatus::Playing {
            pos + elapsed.as_secs_f64()
        } else {
            pos
        };
        (s.position - expected).abs() > SEEK_TOLERANCE
            || (s.status == PlaybackStatus::Playing && elapsed >= TIMELINE_INTERVAL)
    }

    fn set_timeline(&mut self, s: &NowPlaying) -> windows::core::Result<()> {
        let duration = s.media.as_ref().map_or(0.0, |m| m.duration);
        let timeline = SystemMediaTransportControlsTimelineProperties::new()?;
        timeline.SetStartTime(span(0.0))?;
        timeline.SetMinSeekTime(span(0.0))?;
        timeline.SetEndTime(span(duration))?;
        timeline.SetMaxSeekTime(span(duration))?;
        timeline.SetPosition(span(s.position.min(duration)))?;
        self.smtc.UpdateTimelineProperties(&timeline)?;
        self.timeline_at = Some((Instant::now(), s.position));
        Ok(())
    }
}

/// WinRT `TimeSpan`: units of 100 ns.
fn span(seconds: f64) -> TimeSpan {
    TimeSpan {
        Duration: (seconds.max(0.0) * 10_000_000.0) as i64,
    }
}
