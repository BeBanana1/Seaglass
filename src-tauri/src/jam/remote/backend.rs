//! The app's side of the remote control: the `Remote` that `api.rs` calls, wired to the
//! player and the TIDAL client.

use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use super::api::{Command, Remote};
use super::{History, SEARCH_LIMIT};
use crate::content::filtered;
use crate::handoff::history::{SyncRequest, SyncResponse};
use crate::i18n::tr;
use crate::player::{Cmd, Handoff, PlayerState, QueueView};
use crate::tidal::TidalClient;
use crate::tidal::models::{PlaylistSummary, SearchResults, Track};

pub(super) struct AppRemote {
    pub(super) tidal: Arc<TidalClient>,
    pub(super) player: mpsc::Sender<Cmd>,
    pub(super) history: Option<Arc<History>>,
}

fn stopped() -> String {
    tr("lecteur arrêté", "player stopped").to_owned()
}

impl AppRemote {
    async fn send(&self, cmd: Cmd) -> Result<(), String> {
        self.player.send(cmd).await.map_err(|_| stopped())
    }
}

/// Translates a remote control command for the player actor.
fn to_cmd(command: Command) -> Cmd {
    match command {
        Command::SetPaused(paused) => Cmd::SetPaused(paused),
        Command::Next => Cmd::Next,
        Command::Previous => Cmd::Previous,
        Command::Seek(seconds) => Cmd::Seek(seconds),
        Command::Volume(level) => Cmd::SetVolume(level),
        Command::Muted(muted) => Cmd::SetMuted(muted),
        Command::Shuffle(enabled) => Cmd::SetShuffle(enabled),
        Command::Repeat(mode) => Cmd::SetRepeat(mode),
        Command::Jump { part, index, id } => Cmd::PlayFromQueue { part, index, id },
        Command::Remove(id) => Cmd::RemoveFromQueueIds(vec![id]),
        Command::Play {
            ids,
            shuffle,
            source,
        } => Cmd::PlayTracks {
            ids,
            start: 0,
            shuffle,
            source: source.and_then(|s| s.sanitize(crate::commands::settings::pins::is_uuid)),
        },
        Command::Enqueue { ids, next } => Cmd::Enqueue { ids, next },
    }
}

impl Remote for AppRemote {
    async fn state(&self) -> Result<(PlayerState, QueueView), String> {
        let (tx, rx) = oneshot::channel();
        self.send(Cmd::GetState(tx)).await?;
        let player = rx.await.map_err(|_| stopped())?;
        let (tx, rx) = oneshot::channel();
        self.send(Cmd::GetQueue(tx)).await?;
        let queue = rx.await.map_err(|_| stopped())?;
        Ok((player, queue))
    }

    async fn run(&self, command: Command) -> Result<(), String> {
        self.send(to_cmd(command)).await
    }

    async fn search(&self, query: String) -> Result<SearchResults, String> {
        self.tidal
            .search(&query, SEARCH_LIMIT)
            .await
            .map(filtered)
            .map_err(|e| e.to_string())
    }

    async fn album(&self, id: u64) -> Result<(String, Vec<Track>), String> {
        let page = self.tidal.album(id).await.map_err(|e| e.to_string())?;
        let page = filtered(page);
        Ok((page.album.title, page.tracks))
    }

    async fn playlist(&self, uuid: String) -> Result<(String, Vec<Track>), String> {
        let page = self
            .tidal
            .playlist(&uuid)
            .await
            .map_err(|e| e.to_string())?;
        let page = filtered(page);
        let tracks = page.tracks.into_iter().map(|e| e.track).collect();
        Ok((page.playlist.title, tracks))
    }

    async fn playlists(&self) -> Result<Vec<PlaylistSummary>, String> {
        self.tidal
            .library_playlists()
            .await
            .map(|list| list.items)
            .map_err(|e| e.to_string())
    }

    async fn take_handoff(&self) -> Result<Option<Handoff>, String> {
        let (tx, rx) = oneshot::channel();
        self.send(Cmd::TakeHandoff(tx)).await?;
        rx.await.map_err(|_| stopped())
    }

    async fn give_handoff(&self, handoff: Handoff) -> Result<bool, String> {
        let (tx, rx) = oneshot::channel();
        self.send(Cmd::ApplyHandoff(Box::new(handoff), tx)).await?;
        rx.await.map_err(|_| stopped())
    }

    async fn sync_plays(&self, request: SyncRequest) -> Result<SyncResponse, String> {
        let history = self
            .history
            .clone()
            .ok_or_else(|| "historique indisponible".to_owned())?;
        tokio::task::spawn_blocking(move || {
            let (response, added) = crate::handoff::history::answer(&history.store, &request)
                .map_err(|e| e.to_string())?;
            if added > 0 {
                tracing::info!("History: {added} listens received from the phone");
                (history.on_added)(added);
            }
            Ok(response)
        })
        .await
        .map_err(|e| e.to_string())?
    }
}
