//! Owner API (remote control, #336): every player command, behind an owner code distinct
//! from the Jam guests' one. The commands are read from the request in `api/command.rs`.
//!
//! Separate from the guest API (`jam/api.rs`) and served on another port: a Jam page cannot
//! call any of these commands, and a Jam's code opens nothing here.
//! Nothing writes to the account (no like, no playlist changed): "My playlists" is only
//! read.

use std::future::Future;
use std::net::IpAddr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde::de::DeserializeOwned;
use tokio::sync::Notify;

use super::view::{RemotePlaylist, RemoteView};
use crate::handoff::history::{SyncRequest, SyncResponse};
use crate::jam::api::{BACKEND_TIMEOUT, MAX_QUERY_CHARS};
use crate::jam::guard::{Action, Limits, code_matches};
use crate::jam::http::Response;
use crate::jam::view::{GuestAlbum, GuestTrack};
use crate::player::{Handoff, PlaySource, PlayerState, QueuePart, QueueView, Repeat};
use crate::tidal::models::{PlaylistSummary, SearchResults, Track, TrackId};
use command::{command, context};

/// Length of the owner code: 32¹² ≈ 1.2 × 10¹⁸ codes (60 bits), against 6 characters for
/// a Jam. With 10 attempts per quarter hour and per address, there is no point guessing
/// it.
pub const OWNER_CODE_LEN: usize = 12;
const MAX_TRACKS: usize = 30;
const MAX_ALBUMS: usize = 20;
const MAX_PLAYLISTS: usize = 20;
/// "My playlists" shown at most.
const MAX_OWN_PLAYLISTS: usize = 200;
/// Tracks read at most from an album or a playlist (bound of `play_tracks`).
const MAX_CONTEXT: usize = 10_000;
/// Key of the limits: a single owner, the address bounds the rest.
const OWNER: &str = "owner";
/// How long a phone can learn the relay route after "Link a phone" (#62).
pub const PAIRING_WINDOW: Duration = Duration::from_secs(10 * 60);

/// What the remote control asks of the player, translated into a `Cmd` by the app.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    SetPaused(bool),
    Next,
    Previous,
    Seek(f64),
    Volume(u8),
    Muted(bool),
    Shuffle(bool),
    Repeat(Repeat),
    /// Plays a queue track as the page shows it.
    Jump {
        part: QueuePart,
        index: usize,
        id: TrackId,
    },
    /// Removes an entry from the manual queue (the first one with this id).
    Remove(TrackId),
    Play {
        ids: Vec<TrackId>,
        /// As in `Cmd::PlayTracks`: `None` keeps the current mode (a single track).
        shuffle: Option<bool>,
        source: Option<PlaySource>,
    },
    Enqueue {
        ids: Vec<TrackId>,
        next: bool,
    },
}

/// What the API asks of the app; replaced by a fake in the tests.
pub trait Remote: Send + Sync + 'static {
    fn state(&self) -> impl Future<Output = Result<(PlayerState, QueueView), String>> + Send;
    fn run(&self, command: Command) -> impl Future<Output = Result<(), String>> + Send;
    fn search(&self, query: String) -> impl Future<Output = Result<SearchResults, String>> + Send;
    /// Title and playable tracks of an album.
    fn album(&self, id: u64) -> impl Future<Output = Result<(String, Vec<Track>), String>> + Send;
    /// Title and playable tracks of a playlist.
    fn playlist(
        &self,
        uuid: String,
    ) -> impl Future<Output = Result<(String, Vec<Track>), String>> + Send;
    /// The account's playlists (read only).
    fn playlists(&self) -> impl Future<Output = Result<Vec<PlaylistSummary>, String>> + Send;
    /// Snapshot of the playback then pause (#414); `None` if nothing is playing.
    fn take_handoff(&self) -> impl Future<Output = Result<Option<Handoff>, String>> + Send;
    /// Resumes a handed-off playback, already checked.
    fn give_handoff(&self, handoff: Handoff) -> impl Future<Output = Result<bool, String>> + Send;
    /// Exchange of the listening history with the phone (#430), request already checked.
    fn sync_plays(
        &self,
        request: SyncRequest,
    ) -> impl Future<Output = Result<SyncResponse, String>> + Send;
}

/// Running remote control: code, limits, state rereading.
pub struct Owner {
    code: Mutex<String>,
    limits: Mutex<Limits>,
    /// Remote control page and commands served.
    remote: AtomicBool,
    /// Playback handoffs accepted (#414).
    handoff: AtomicBool,
    /// Read the state again without waiting (after a command, a new page).
    pub refresh: Notify,
    /// Online address and key of the relay tunnel (#430), given to the linked phone on the
    /// local network; `None` until the tunnel is open.
    relay: Mutex<Option<super::tunnel::RelayRoute>>,
    /// End of the pairing window (#62): `api/relay` only answers before it.
    pairing_until: Mutex<Option<Instant>>,
}

impl Owner {
    pub fn new(code: String) -> Self {
        Self {
            code: Mutex::new(code),
            limits: Mutex::new(Limits::default()),
            remote: AtomicBool::new(true),
            handoff: AtomicBool::new(false),
            refresh: Notify::new(),
            relay: Mutex::new(None),
            pairing_until: Mutex::new(None),
        }
    }

    /// Opens the pairing window for `length` from now (#62); a new opening replaces it.
    pub fn open_pairing(&self, length: Duration) {
        if let Ok(mut until) = self.pairing_until.lock() {
            *until = Some(Instant::now() + length);
        }
    }

    /// True while a phone can learn the relay route.
    pub fn pairing_open(&self) -> bool {
        self.pairing_until
            .lock()
            .ok()
            .and_then(|until| *until)
            .is_some_and(|until| Instant::now() < until)
    }

    /// What the server serves: the remote control, handoffs, or both.
    pub fn set_features(&self, remote: bool, handoff: bool) {
        self.remote.store(remote, Ordering::Relaxed);
        self.handoff.store(handoff, Ordering::Relaxed);
    }

    pub fn remote_enabled(&self) -> bool {
        self.remote.load(Ordering::Relaxed)
    }

    pub fn handoff_enabled(&self) -> bool {
        self.handoff.load(Ordering::Relaxed)
    }

    pub fn set_relay(&self, route: Option<super::tunnel::RelayRoute>) {
        if let Ok(mut relay) = self.relay.lock() {
            *relay = route;
        }
    }

    /// `POST api/relay` (#430): what the phone needs to reach this PC through the relay,
    /// encrypted. Never served through the tunnel itself. Only during the pairing window
    /// (#62), otherwise 403 `pairing`: the owner code travels in clear on the local
    /// network, the tunnel key must not be one request away from whoever read it. 404
    /// `relay` if the tunnel is not open.
    fn relay_route(&self) -> Response {
        if !self.handoff_enabled() {
            return Response::error(403, "handoff");
        }
        if !self.pairing_open() {
            return Response::error(403, "pairing");
        }
        match self.relay.lock().ok().and_then(|r| r.clone()) {
            Some(route) => match serde_json::to_string(&route) {
                Ok(json) => Response::new(200, "application/json", json),
                Err(e) => backend_error(&e.to_string()),
            },
            None => Response::error(404, "relay"),
        }
    }

    /// New code: pages opened with the old one are refused from their next call.
    pub fn set_code(&self, code: String) {
        if let Ok(mut current) = self.code.lock() {
            *current = code;
        }
    }

    /// Blocked address or wrong code (counted as a failed attempt): the error answer.
    pub fn check_code(&self, ip: IpAddr, given: &str) -> Result<(), Response> {
        let (Ok(code), Ok(mut limits)) = (self.code.lock(), self.limits.lock()) else {
            return Err(Response::error(502, "backend"));
        };
        let now = Instant::now();
        if limits.blocked(ip, now) {
            return Err(Response::error(429, "blocked"));
        }
        if !code_matches(&code, given) {
            limits.fail(ip, now);
            return Err(Response::error(401, "code"));
        }
        Ok(())
    }

    pub(super) fn allow(&self, ip: IpAddr, action: Action) -> bool {
        self.limits
            .lock()
            .map(|mut l| l.allow(ip, OWNER, action, Instant::now()))
            .unwrap_or(false)
    }

    /// State of the remote control as JSON, `None` if the player does not answer or if the
    /// remote control is off (handoffs only).
    pub async fn state_json<R: Remote>(&self, remote: &R) -> Option<String> {
        if !self.remote_enabled() {
            return None;
        }
        let (player, queue) = tokio::time::timeout(BACKEND_TIMEOUT, remote.state())
            .await
            .ok()?
            .ok()?;
        Some(serde_json::json!(RemoteView::new(&player, &queue)).to_string())
    }

    /// `POST /api/<…>` call from the page, `path` included (`/api/next`).
    pub async fn api<R: Remote>(
        &self,
        path: &str,
        code: Option<&str>,
        body: &[u8],
        ip: IpAddr,
        remote: &R,
    ) -> Response {
        if let Err(response) = self.check_code(ip, code.unwrap_or("")) {
            return response;
        }
        if path == "/api/join" {
            return ok();
        }
        if path == "/api/relay" {
            return self.relay_route();
        }
        if let Some(response) = handoff::route(self, path, body, ip, remote).await {
            return response;
        }
        if let Some(response) = history::route(self, path, body, ip, remote).await {
            return response;
        }
        // Handoffs only: nothing of the remote control is served.
        if !self.remote_enabled() {
            return Response::not_found();
        }
        match path {
            "/api/state" => {
                return match self.state_json(remote).await {
                    Some(json) => Response::new(200, "application/json", json),
                    None => Response::error(502, "backend"),
                };
            }
            "/api/search" => return self.search(body, ip, remote).await,
            _ => {}
        }
        let command = match command(path, body) {
            Ok(Some(command)) => Ok(command),
            Ok(None) => match path {
                "/api/playlists" => return self.playlists(ip, remote).await,
                "/api/play-album" | "/api/play-playlist" => {
                    if !self.allow(ip, Action::Command) {
                        return Response::error(429, "slow");
                    }
                    context(path, body, remote).await
                }
                _ => return Response::not_found(),
            },
            Err(kind) => Err(Response::error(400, kind)),
        };
        let command = match command {
            Ok(command) => command,
            Err(response) => return response,
        };
        // `play-album` and `play-playlist` were already counted before calling TIDAL.
        let counted = matches!(path, "/api/play-album" | "/api/play-playlist");
        if !counted && !self.allow(ip, Action::Command) {
            return Response::error(429, "slow");
        }
        if let Err(e) = remote.run(command).await {
            return backend_error(&e);
        }
        self.refresh.notify_one();
        ok()
    }

    async fn search<R: Remote>(&self, body: &[u8], ip: IpAddr, remote: &R) -> Response {
        let Ok(body) = parse::<SearchBody>(body) else {
            return Response::error(400, "body");
        };
        let query = body.query.trim();
        if query.is_empty() {
            return Response::json(
                200,
                &serde_json::json!({ "tracks": [], "albums": [], "playlists": [] }),
            );
        }
        if query.chars().count() > MAX_QUERY_CHARS {
            return Response::error(400, "query");
        }
        if !self.allow(ip, Action::Search) {
            return Response::error(429, "slow");
        }
        let hits = match remote.search(query.to_owned()).await {
            Ok(hits) => hits,
            Err(e) => return backend_error(&e),
        };
        let tracks: Vec<GuestTrack> = hits
            .tracks
            .iter()
            .take(MAX_TRACKS)
            .map(GuestTrack::from)
            .collect();
        let albums: Vec<GuestAlbum> = hits
            .albums
            .iter()
            .take(MAX_ALBUMS)
            .map(GuestAlbum::from)
            .collect();
        let playlists: Vec<RemotePlaylist> = hits
            .playlists
            .iter()
            .take(MAX_PLAYLISTS)
            .map(RemotePlaylist::from)
            .collect();
        Response::json(
            200,
            &serde_json::json!({ "tracks": tracks, "albums": albums, "playlists": playlists }),
        )
    }

    async fn playlists<R: Remote>(&self, ip: IpAddr, remote: &R) -> Response {
        if !self.allow(ip, Action::Search) {
            return Response::error(429, "slow");
        }
        match remote.playlists().await {
            Ok(list) => {
                let list: Vec<RemotePlaylist> = list
                    .iter()
                    .take(MAX_OWN_PLAYLISTS)
                    .map(RemotePlaylist::from)
                    .collect();
                Response::json(200, &serde_json::json!(list))
            }
            Err(e) => backend_error(&e),
        }
    }
}

fn parse<T: DeserializeOwned>(body: &[u8]) -> Result<T, &'static str> {
    serde_json::from_slice(body).map_err(|_| "body")
}

fn ok() -> Response {
    Response::json(200, &serde_json::json!({ "ok": true }))
}

/// The detail stays in the log: the page only shows a generic message.
pub(super) fn backend_error(e: &str) -> Response {
    tracing::warn!("Remote control: {e}");
    Response::error(502, "backend")
}

#[derive(Deserialize)]
struct SearchBody {
    query: String,
}

mod command;
mod handoff;
mod history;
pub(super) use history::PATH as HISTORY_PATH;
#[cfg(test)]
mod tests;
