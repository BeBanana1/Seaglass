//! One stream per account (#35): the player tells TIDAL's Pushkin service when it starts
//! playing on an explicit Play (the other clients of the account pause), and pauses when
//! another client does the same. The connection itself is `tidal::pushkin`.

use std::sync::Arc;

use tokio::sync::mpsc;

use super::actor::{Cmd, Player};
use super::blind::BlindCmd;
use crate::i18n::{tr, trf};
use crate::tidal::TidalClient;
use crate::tidal::models::PlaybackTaken;
use crate::tidal::pushkin::{Playback, Pushkin};

/// What the player last told Pushkin, and the claim waiting for playback to start.
pub(super) struct Takeover {
    pushkin: Pushkin,
    /// An explicit Play is under way: sent as a claim once playback starts.
    claim: bool,
    playing: bool,
}

impl Takeover {
    /// Starts the connection task; takeovers come back to the actor as `Cmd`.
    pub(super) fn start(tidal: Arc<TidalClient>, tx: mpsc::Sender<Cmd>) -> Self {
        let (pushkin, mut taken) = Pushkin::start(tidal);
        tauri::async_runtime::spawn(async move {
            while let Some(event) = taken.recv().await {
                if tx.send(Cmd::PlaybackTaken(event)).await.is_err() {
                    return;
                }
            }
        });
        Self {
            pushkin,
            claim: false,
            playing: false,
        }
    }

    /// After each command or event: what changed goes to Pushkin. A claim only goes out
    /// once audio plays, so a Play that fails to start pauses nobody.
    fn sync(&mut self, playing: bool) {
        if playing && self.claim {
            self.claim = false;
            self.playing = true;
            self.pushkin.send(Playback::Claim);
        } else if playing != self.playing {
            self.playing = playing;
            self.pushkin.send(if playing {
                Playback::Playing
            } else {
                Playback::Stopped
            });
        }
    }
}

impl Player {
    /// Audio is playing here.
    fn playing(&self) -> bool {
        self.loaded && !self.state.paused
    }

    /// Commands whose result is playing, from the user, the media keys, Jam guests or the
    /// remote control. Never the player's own moves (gapless, next track at the end, error
    /// retry, automatic radio, engine recovery). Resume on launch counts: otherwise this
    /// device and another one would play together, silently.
    pub(super) fn note_claim(&mut self, cmd: &Cmd) {
        let claims = match cmd {
            Cmd::PlayTracks { .. }
            | Cmd::PlayFromQueue { .. }
            | Cmd::Next
            | Cmd::Previous
            | Cmd::SetPaused(false)
            | Cmd::ApplyHandoff(..)
            | Cmd::ResumeOnLaunch
            | Cmd::AnswerHiresOffer(true)
            | Cmd::BlindTest(BlindCmd::Start { .. }) => true,
            Cmd::TogglePause => !self.playing(),
            _ => false,
        };
        if claims {
            self.takeover.claim = true;
        }
    }

    pub(super) fn sync_takeover(&mut self) {
        let playing = self.playing();
        self.takeover.sync(playing);
    }

    /// Another client of the account started playing: pause, once. While nothing plays
    /// here (already paused, handed off), the notification changes nothing.
    pub(super) async fn on_playback_taken(&mut self, taken: PlaybackTaken) {
        self.takeover.claim = false;
        if !self.playing() {
            tracing::debug!("playback taken by another client, nothing playing here");
            return;
        }
        tracing::info!(
            "playback taken by another client ({}), pausing",
            taken.device.as_deref().unwrap_or("unnamed")
        );
        self.set_paused(true).await;
        let message = match &taken.device {
            Some(device) => trf!(
                "Lecture mise en pause : le compte joue sur un autre appareil ({device})",
                "Playback paused: the account is playing on another device ({device})"
            ),
            None => tr(
                "Lecture mise en pause : le compte joue sur un autre appareil",
                "Playback paused: the account is playing on another device",
            )
            .to_owned(),
        };
        self.toast("info", message);
    }
}
