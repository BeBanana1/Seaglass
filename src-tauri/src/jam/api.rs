//! Core of the guest API (#25), shared by the local Jam (`server.rs`) and the relay
//! (`relay.rs`): access code, limits per guest and per address (#256), offered tracks and
//! albums, guest additions, the host's rules and bans (#288), the Jam's cap in the queue
//! (#256), volume (#449), lyrics (#448), vote to skip (#433), state. Both transports only
//! deliver requests here and send back the responses.

mod adds;
mod blind;
mod lyrics;
mod search;
mod skip;
mod volume;

pub use skip::GuestSkip;

use std::collections::HashMap;
use std::future::Future;
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Deserialize;
use tokio::sync::Notify;

use super::clock::{Anchor, anchor, now_ms};
use super::guard::{Action, Limits, Offered, code_matches};
use super::guests::{Guests, sanitize_token};
use super::http::Response;
use super::members::{GuestInfo, owner_key};
use super::rules::JamRules;
use super::view::GuestState;
use crate::player::blind::{Answer, BlindTestView, Refusal};
use crate::player::volume::Volume;
use crate::tidal::models::{AlbumSummary, Lyrics, Track, TrackId};

/// Timeout of a response that waits for TIDAL or the player.
pub const BACKEND_TIMEOUT: Duration = Duration::from_secs(15);
/// A pasted link (Apple Music, Spotify with its parameters) is often over 100 characters.
pub const MAX_QUERY_CHARS: usize = 300;
/// Most tracks of an album added: a box set does not fill the queue (#223).
pub const MAX_ALBUM_TRACKS: usize = 30;
/// Upcoming tracks shown to guests.
pub const MAX_QUEUE: usize = 20;

/// The player's state as the Jam sees it.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub now: Option<Track>,
    pub paused: bool,
    /// Position in the current track, in seconds (synced lyrics, #448).
    pub position: f64,
    /// Manual queue.
    pub manual: Vec<Track>,
    /// Rest of the context.
    pub upcoming: Vec<Track>,
    /// Blind test in progress (#434), track hidden until the round is revealed.
    pub blind: Option<BlindTestView>,
    /// The host's volume, shown to guests only if they may set it (#449).
    pub volume: Volume,
}

impl Snapshot {
    /// Blind test open to the Jam's guests (#441); a solo game does not concern them.
    pub fn jam_game(&self) -> Option<&BlindTestView> {
        self.blind.as_ref().filter(|b| b.party.jam)
    }
}

/// Results of a Jam search.
#[derive(Debug, Clone, Default)]
pub struct Hits {
    pub tracks: Vec<Track>,
    pub albums: Vec<AlbumSummary>,
}

/// What the server asks of the app; replaced by a fake in the tests.
pub trait Backend: Send + Sync + 'static {
    fn search(&self, query: String) -> impl Future<Output = Result<Hits, String>> + Send;
    /// Tracks or albums of a pasted link (TIDAL, Deezer, Spotify, Apple Music, YouTube,
    /// #229); called only if `crate::foreign::find` recognizes a link.
    fn link(&self, text: String) -> impl Future<Output = Result<Hits, String>> + Send;
    /// Playable tracks of an album, in order (#223).
    fn album_tracks(&self, id: u64) -> impl Future<Output = Result<Vec<Track>, String>> + Send;
    /// Appends to the manual queue, in the given order.
    fn enqueue(&self, ids: Vec<TrackId>) -> impl Future<Output = Result<(), String>> + Send;
    /// Removes one entry per id from the manual queue (#223).
    fn remove(&self, ids: Vec<TrackId>) -> impl Future<Output = Result<(), String>> + Send;
    /// Play (`false`) or pause (`true`), asked by a guest (#222).
    fn set_paused(&self, paused: bool) -> impl Future<Output = Result<(), String>> + Send;
    /// Volume asked by a guest (#449), already brought down to the host's cap.
    fn set_volume(&self, level: u8) -> impl Future<Output = Result<(), String>> + Send;
    /// Skips the current track: the guests voted for it (#433). The host is told.
    fn skip_by_vote(&self) -> impl Future<Output = Result<(), String>> + Send;
    fn state(&self) -> impl Future<Output = Result<Snapshot, String>> + Send;
    /// Lyrics of a track (#448), `None` if it has none.
    fn lyrics(&self, id: TrackId) -> impl Future<Output = Result<Option<Lyrics>, String>> + Send;
    /// A guest's answer to the blind test (#441): their player id, or the game's
    /// refusal.
    fn blind_answer(
        &self,
        key: String,
        name: Option<String>,
        round: usize,
        answer: Answer,
    ) -> impl Future<Output = Result<Result<u32, Refusal>, String>> + Send;
    /// The guests' additions changed: the host's UI shows the names in the queue.
    fn guests_changed(&self, guests: &Guests);
}

struct Session {
    code: String,
    limits: Limits,
    offered: Offered,
    offered_albums: Offered,
}

/// State of a Jam on the host's side, whatever the transport.
pub struct Core {
    session: Mutex<Session>,
    guests: Arc<Mutex<Guests>>,
    /// The host's rules (#222, #288), kept from one Jam to the next.
    rules: Mutex<JamRules>,
    /// The host can suspend additions for the rest of the Jam (#288).
    adds_open: AtomicBool,
    /// Banning a guest also blocks their address (local network only, #288).
    ban_ips: bool,
    /// One addition at a time: the per-guest limit and duplicates are checked against the
    /// queue that follows the previous addition.
    add_turn: tokio::sync::Mutex<()>,
    /// Lyrics of the last track a guest asked for (#448).
    lyrics: tokio::sync::Mutex<Option<lyrics::CachedLyrics>>,
    /// Start point of the current track sent to guests (#448, `clock.rs`).
    clock: Mutex<Option<Anchor>>,
    /// Votes to skip the current track (#433).
    skip_votes: Mutex<skip::SkipVotes>,
    /// Read the state again without waiting (a guest's addition, a new page, a pause).
    pub refresh: Notify,
}

impl Core {
    #[cfg(test)]
    pub fn new(code: String, guests: Arc<Mutex<Guests>>) -> Self {
        Self::with_rules(code, guests, JamRules::default(), false)
    }

    pub fn with_rules(
        code: String,
        guests: Arc<Mutex<Guests>>,
        rules: JamRules,
        ban_ips: bool,
    ) -> Self {
        Self {
            session: Mutex::new(Session {
                code,
                limits: Limits::default(),
                offered: Offered::default(),
                offered_albums: Offered::default(),
            }),
            guests,
            rules: Mutex::new(rules.sanitized()),
            adds_open: AtomicBool::new(true),
            ban_ips,
            add_turn: tokio::sync::Mutex::new(()),
            lyrics: tokio::sync::Mutex::new(None),
            clock: Mutex::new(None),
            skip_votes: Mutex::new(skip::SkipVotes::default()),
            refresh: Notify::new(),
        }
    }

    pub fn rules(&self) -> JamRules {
        self.rules.lock().map(|r| *r).unwrap_or_default()
    }

    /// The host's choice, pushed at once to open pages (their buttons and messages change).
    pub fn set_rules(&self, rules: JamRules) {
        if let Ok(mut current) = self.rules.lock() {
            *current = rules.sanitized();
        }
        self.refresh.notify_one();
    }

    pub fn controls(&self) -> bool {
        self.rules().guest_controls
    }

    #[cfg(test)]
    pub fn set_controls(&self, enabled: bool) {
        self.set_rules(JamRules {
            guest_controls: enabled,
            ..self.rules()
        });
    }

    pub fn adds_open(&self) -> bool {
        self.adds_open.load(Ordering::Relaxed)
    }

    pub fn set_adds_open(&self, open: bool) {
        self.adds_open.store(open, Ordering::Relaxed);
        self.refresh.notify_one();
    }

    /// Guests who added during the Jam, for the host's UI.
    pub fn members(&self) -> Vec<GuestInfo> {
        self.guests
            .lock()
            .map(|g| g.members.list())
            .unwrap_or_default()
    }

    /// Bans a guest (their waiting tracks are removed) or lets them back in; false if the
    /// guest does not exist.
    pub async fn set_banned<B: Backend>(&self, id: u32, banned: bool, backend: &B) -> bool {
        let ids = {
            let Ok(mut guests) = self.guests.lock() else {
                return false;
            };
            let Some(key) = guests.members.set_banned(id, banned, self.ban_ips) else {
                return false;
            };
            let ids = if banned {
                guests.take_all(&key)
            } else {
                Vec::new()
            };
            backend.guests_changed(&guests);
            ids
        };
        if !ids.is_empty()
            && let Err(e) = backend.remove(ids.into_iter().map(TrackId).collect()).await
        {
            tracing::warn!("Jam: {e}");
        }
        self.refresh.notify_one();
        true
    }

    fn is_banned(&self, key: &str, ip: IpAddr) -> bool {
        self.guests
            .lock()
            .map(|g| g.members.is_banned(key, ip))
            .unwrap_or(false)
    }

    /// Guest name of each track added by the Jam, for the host's UI.
    pub fn guest_names(&self) -> HashMap<u64, String> {
        self.guests.lock().map(|g| g.snapshot()).unwrap_or_default()
    }

    /// Blocked address or wrong code (counted as a failed attempt): the error response.
    pub fn check_code(&self, ip: IpAddr, given: &str) -> Result<(), Response> {
        let Ok(mut s) = self.session.lock() else {
            return Err(Response::error(502, "backend"));
        };
        let now = Instant::now();
        if s.limits.blocked(ip, now) {
            return Err(Response::error(429, "blocked"));
        }
        if !code_matches(&s.code, given) {
            s.limits.fail(ip, now);
            return Err(Response::error(401, "code"));
        }
        Ok(())
    }

    /// Action counted for the guest (`key`, `owner_key`) and for their address (#256).
    fn allow(&self, ip: IpAddr, key: &str, action: Action) -> bool {
        self.session
            .lock()
            .map(|mut s| s.limits.allow(ip, key, action, Instant::now()))
            .unwrap_or(false)
    }

    /// The guests' state as JSON, `None` if the player does not answer.
    pub async fn state_json<B: Backend>(&self, backend: &B) -> Option<String> {
        let snapshot = tokio::time::timeout(BACKEND_TIMEOUT, backend.state())
            .await
            .ok()?
            .ok()?;
        let mut state = {
            let guests = self.guests.lock().ok()?;
            GuestState::new(&snapshot, &guests, &self.rules(), self.adds_open())
        };
        self.add_clock(&mut state, &snapshot);
        state.skip = self.skip_view(
            snapshot.now.as_ref().map(|t| t.id.0),
            snapshot.jam_game().is_some(),
        );
        Some(serde_json::json!(state).to_string())
    }

    /// Position and known lyrics of the current track (#448), except during a blind test.
    fn add_clock(&self, state: &mut GuestState, snapshot: &Snapshot) {
        let Some(track) = snapshot.now.as_ref().map(|t| t.id.0) else {
            return;
        };
        if snapshot.blind.is_some() {
            return;
        }
        state.lyrics = self.known_lyrics(track);
        if snapshot.paused {
            state.position = Some((snapshot.position.max(0.0) * 1000.0).round() as u64);
            return;
        }
        if let Ok(mut clock) = self.clock.lock() {
            let next = anchor(*clock, track, snapshot.position, now_ms());
            *clock = Some(next);
            state.started_at = Some(next.started_at);
        }
    }

    /// A guest's `POST /api/<…>` call, `path` included (`/api/search`).
    pub async fn api<B: Backend>(
        &self,
        path: &str,
        code: Option<&str>,
        body: &[u8],
        ip: IpAddr,
        backend: &B,
    ) -> Response {
        if let Err(response) = self.check_code(ip, code.unwrap_or("")) {
            return response;
        }
        // Browser token, sent with every request since #288: a banned guest is refused
        // everywhere, and the page tells them.
        let token = serde_json::from_slice::<GuestOnly>(body)
            .ok()
            .and_then(|b| b.guest)
            .and_then(|t| sanitize_token(&t));
        let key = owner_key(token.as_deref(), ip);
        if self.is_banned(&key, ip) {
            return Response::error(403, "banned");
        }
        match path {
            "/api/join" => Response::json(200, &serde_json::json!({ "ok": true })),
            "/api/state" => match self.state_json(backend).await {
                Some(json) => Response::new(200, "application/json", json),
                None => Response::error(502, "backend"),
            },
            "/api/search" => self.search(body, ip, &key, backend).await,
            "/api/add" => self.add(body, ip, &key, backend).await,
            "/api/add-album" => self.add_album(body, ip, &key, backend).await,
            "/api/remove" => self.remove(body, ip, &key, backend).await,
            "/api/pause" => self.pause(body, ip, &key, backend).await,
            "/api/blind-answer" => self.blind_answer(body, ip, &key, backend).await,
            "/api/volume" => self.volume(body, ip, &key, backend).await,
            "/api/skip" => self.skip(body, ip, &key, backend).await,
            "/api/lyrics" => self.lyrics(ip, &key, backend).await,
            _ => Response::not_found(),
        }
    }

    /// Explicit play or pause (not a toggle: two guests who press at the same time get the
    /// same result).
    async fn pause<B: Backend>(&self, body: &[u8], ip: IpAddr, key: &str, backend: &B) -> Response {
        let Ok(body) = serde_json::from_slice::<PauseBody>(body) else {
            return Response::error(400, "body");
        };
        if !self.controls() {
            return Response::error(403, "controls");
        }
        if !self.allow(ip, key, Action::Control) {
            return Response::error(429, "slow");
        }
        if let Err(e) = backend.set_paused(body.paused).await {
            return backend_error(&e);
        }
        self.refresh.notify_one();
        Response::json(200, &serde_json::json!({ "ok": true }))
    }
}

#[derive(Deserialize)]
struct PauseBody {
    paused: bool,
}

/// Browser token, read in every request (#288).
#[derive(Deserialize)]
struct GuestOnly {
    #[serde(default)]
    guest: Option<String>,
}

/// The detail stays in the host's log: the page only shows a generic message.
fn backend_error(e: &str) -> Response {
    tracing::warn!("Jam: {e}");
    Response::error(502, "backend")
}

#[cfg(test)]
mod tests;
