//! The app's side of the Jam guests: the `Backend` that `api.rs` calls (TIDAL search,
//! queue, pause, blind test), wired to the player.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot};

use super::api::{Backend, Hits, Snapshot};
use super::guests::Guests;
use super::{GUESTS_EVENT, GuestsUpdate, SEARCH_LIMIT};
use crate::content::filtered;
use crate::i18n::{tr, trf};
use crate::player::Cmd;
use crate::player::blind::{Answer, BlindCmd, Refusal};
use crate::tidal::TidalClient;
use crate::tidal::models::{Lyrics, Track, TrackId};

pub(super) struct AppBackend {
    pub(super) app: AppHandle,
    pub(super) tidal: Arc<TidalClient>,
    pub(super) player: mpsc::Sender<Cmd>,
}

fn stopped() -> String {
    tr("lecteur arrêté", "player stopped").to_owned()
}

impl Backend for AppBackend {
    async fn search(&self, query: String) -> Result<Hits, String> {
        self.tidal
            .search(&query, SEARCH_LIMIT)
            .await
            .map(filtered)
            .map(|r| Hits {
                tracks: r.tracks,
                albums: r.albums,
            })
            .map_err(|e| e.to_string())
    }

    async fn link(&self, text: String) -> Result<Hits, String> {
        match crate::foreign::resolve(&self.tidal, &text).await {
            Some(Ok(found)) => Ok(Hits {
                tracks: filtered(found.tracks),
                albums: filtered(found.albums),
            }),
            Some(Err(e)) => Err(trf!("lien illisible : {}", "unreadable link: {}", e)),
            None => Ok(Hits::default()),
        }
    }

    async fn album_tracks(&self, id: u64) -> Result<Vec<Track>, String> {
        self.tidal
            .album(id)
            .await
            .map(|page| filtered(page.tracks))
            .map_err(|e| e.to_string())
    }

    async fn enqueue(&self, ids: Vec<TrackId>) -> Result<(), String> {
        self.player
            .send(Cmd::Enqueue { ids, next: false })
            .await
            .map_err(|_| stopped())
    }

    async fn remove(&self, ids: Vec<TrackId>) -> Result<(), String> {
        self.player
            .send(Cmd::RemoveFromQueueIds(ids))
            .await
            .map_err(|_| stopped())
    }

    async fn set_paused(&self, paused: bool) -> Result<(), String> {
        self.player
            .send(Cmd::SetPaused(paused))
            .await
            .map_err(|_| stopped())
    }

    async fn set_volume(&self, level: u8) -> Result<(), String> {
        self.player
            .send(Cmd::SetVolume(level))
            .await
            .map_err(|_| stopped())
    }

    async fn skip_by_vote(&self) -> Result<(), String> {
        self.player.send(Cmd::Next).await.map_err(|_| stopped())?;
        tracing::info!("Jam: track skipped by the guests' vote");
        #[derive(Clone, Serialize)]
        struct Toast {
            level: &'static str,
            message: &'static str,
        }
        let toast = Toast {
            level: "info",
            message: tr(
                "Titre passé par le vote des invités de la Jam",
                "Track skipped by the Jam guests' vote",
            ),
        };
        if let Err(e) = self.app.emit("toast", toast) {
            tracing::warn!("Jam: {e}");
        }
        Ok(())
    }

    async fn state(&self) -> Result<Snapshot, String> {
        let (tx, rx) = oneshot::channel();
        self.player
            .send(Cmd::GetState(tx))
            .await
            .map_err(|_| stopped())?;
        let player = rx.await.map_err(|_| stopped())?;
        let (tx, rx) = oneshot::channel();
        self.player
            .send(Cmd::GetQueue(tx))
            .await
            .map_err(|_| stopped())?;
        let queue = rx.await.map_err(|_| stopped())?;
        // The game is only asked for if the state announces one (#441).
        let blind = if player.blind_test {
            let (tx, rx) = oneshot::channel();
            self.player
                .send(Cmd::BlindTest(BlindCmd::Get(tx)))
                .await
                .map_err(|_| stopped())?;
            rx.await.map_err(|_| stopped())?
        } else {
            None
        };
        Ok(Snapshot {
            now: player.track,
            paused: player.paused,
            position: player.time_pos,
            manual: queue.manual,
            upcoming: queue.upcoming,
            blind,
            volume: player.volume,
        })
    }

    async fn lyrics(&self, id: TrackId) -> Result<Option<Lyrics>, String> {
        self.tidal.lyrics(id).await.map_err(|e| e.to_string())
    }

    async fn blind_answer(
        &self,
        key: String,
        name: Option<String>,
        round: usize,
        answer: Answer,
    ) -> Result<Result<u32, Refusal>, String> {
        let (reply, rx) = oneshot::channel();
        let cmd = BlindCmd::GuestAnswer {
            key,
            name,
            round,
            answer,
            reply,
        };
        self.player
            .send(Cmd::BlindTest(cmd))
            .await
            .map_err(|_| stopped())?;
        rx.await.map_err(|_| stopped())
    }

    fn guests_changed(&self, guests: &Guests) {
        let update = GuestsUpdate {
            guests: guests.snapshot(),
            members: guests.members.list(),
        };
        if let Err(e) = self.app.emit(GUESTS_EVENT, update) {
            tracing::warn!("Jam: {e}");
        }
    }
}
