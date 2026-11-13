//! API du propriétaire (télécommande, #336) : toutes les commandes du lecteur, derrière un
//! code de propriétaire distinct de celui des invités de la Jam.
//!
//! Séparée de l'API des invités (`jam/api.rs`) et servie sur un autre port : une page de
//! la Jam ne peut appeler aucune de ces commandes, et le code d'une Jam n'ouvre rien ici.
//! Rien n'écrit dans le compte (pas de like, pas de playlist modifiée) : « Mes playlists »
//! n'est que lu.

use std::future::Future;
use std::net::IpAddr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use serde::Deserialize;
use serde::de::DeserializeOwned;
use tokio::sync::Notify;

use super::view::{RemotePlaylist, RemoteView};
use crate::handoff::history::{SyncRequest, SyncResponse};
use crate::jam::api::{BACKEND_TIMEOUT, MAX_QUERY_CHARS};
use crate::jam::guard::{Action, Limits, code_matches};
use crate::jam::http::Response;
use crate::jam::view::{GuestAlbum, GuestTrack};
use crate::player::volume::Volume;
use crate::player::{Handoff, PlaySource, PlayerState, QueuePart, QueueView, Repeat};
use crate::tidal::models::{PlaylistSummary, SearchResults, Track, TrackId};

/// Longueur du code de propriétaire : 32¹² ≈ 1,2 × 10¹⁸ codes (60 bits), contre 6
/// caractères pour une Jam. Avec 10 essais par quart d'heure et par adresse, inutile de
/// le deviner.
pub const OWNER_CODE_LEN: usize = 12;
const MAX_TRACKS: usize = 30;
const MAX_ALBUMS: usize = 20;
const MAX_PLAYLISTS: usize = 20;
/// « Mes playlists » montrées au plus.
const MAX_OWN_PLAYLISTS: usize = 200;
/// Titres lus au plus d'un album ou d'une playlist (borne de `play_tracks`).
const MAX_CONTEXT: usize = 10_000;
/// Clé des limites : un seul propriétaire, l'adresse borne le reste.
const OWNER: &str = "owner";

/// Ce que la télécommande demande au lecteur, traduit en `Cmd` par l'app.
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
    /// Lire un titre de la file tel que la page l'affiche.
    Jump {
        part: QueuePart,
        index: usize,
        id: TrackId,
    },
    /// Retirer une entrée de la file manuelle (la première de cet identifiant).
    Remove(TrackId),
    Play {
        ids: Vec<TrackId>,
        shuffle: bool,
        source: Option<PlaySource>,
    },
    Enqueue {
        ids: Vec<TrackId>,
        next: bool,
    },
}

/// Ce que l'API demande à l'app ; remplacé par un faux dans les tests.
pub trait Remote: Send + Sync + 'static {
    fn state(&self) -> impl Future<Output = Result<(PlayerState, QueueView), String>> + Send;
    fn run(&self, command: Command) -> impl Future<Output = Result<(), String>> + Send;
    fn search(&self, query: String) -> impl Future<Output = Result<SearchResults, String>> + Send;
    /// Titre et titres lisibles d'un album.
    fn album(&self, id: u64) -> impl Future<Output = Result<(String, Vec<Track>), String>> + Send;
    /// Titre et titres lisibles d'une playlist.
    fn playlist(
        &self,
        uuid: String,
    ) -> impl Future<Output = Result<(String, Vec<Track>), String>> + Send;
    /// Playlists du compte (lecture seule).
    fn playlists(&self) -> impl Future<Output = Result<Vec<PlaylistSummary>, String>> + Send;
    /// Instantané de la lecture puis pause (#414) ; `None` si rien n'est en cours.
    fn take_handoff(&self) -> impl Future<Output = Result<Option<Handoff>, String>> + Send;
    /// Reprend une lecture transférée, déjà vérifiée.
    fn give_handoff(&self, handoff: Handoff) -> impl Future<Output = Result<bool, String>> + Send;
    /// Échange de l'historique d'écoute avec le téléphone (#430), requête déjà vérifiée.
    fn sync_plays(
        &self,
        request: SyncRequest,
    ) -> impl Future<Output = Result<SyncResponse, String>> + Send;
}

/// Télécommande en cours : code, limites, relecture de l'état.
pub struct Owner {
    code: Mutex<String>,
    limits: Mutex<Limits>,
    /// Page et commandes de la télécommande servies.
    remote: AtomicBool,
    /// Transferts de lecture acceptés (#414).
    handoff: AtomicBool,
    /// Relire l'état sans attendre (après une commande, nouvelle page).
    pub refresh: Notify,
    /// Adresse en ligne et clé du tunnel du relais (#430), données au téléphone lié sur
    /// le réseau local ; `None` tant que le tunnel n'est pas ouvert.
    relay: Mutex<Option<super::tunnel::RelayRoute>>,
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
        }
    }

    /// Ce que sert le serveur : la télécommande, les transferts, ou les deux.
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

    /// `POST api/relay` (#430) : de quoi joindre ce PC par le relais, chiffré. Jamais servi
    /// par le tunnel lui-même ; 404 `relay` si le tunnel n'est pas ouvert.
    fn relay_route(&self) -> Response {
        if !self.handoff_enabled() {
            return Response::error(403, "handoff");
        }
        match self.relay.lock().ok().and_then(|r| r.clone()) {
            Some(route) => match serde_json::to_string(&route) {
                Ok(json) => Response::new(200, "application/json", json),
                Err(e) => backend_error(&e.to_string()),
            },
            None => Response::error(404, "relay"),
        }
    }

    /// Nouveau code : les pages ouvertes avec l'ancien sont refusées dès leur prochain appel.
    pub fn set_code(&self, code: String) {
        if let Ok(mut current) = self.code.lock() {
            *current = code;
        }
    }

    /// Adresse bloquée ou code faux (compté comme un essai raté) : la réponse d'erreur.
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

    /// État de la télécommande en JSON, `None` si le lecteur ne répond pas ou si la
    /// télécommande est éteinte (transferts seuls).
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

    /// Appel `POST /api/<…>` de la page, `path` compris (`/api/next`).
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
        // Transferts seuls : rien de la télécommande n'est servi.
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
        // `play-album` et `play-playlist` ont déjà été comptés avant d'appeler TIDAL.
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

/// Commande qui ne demande rien à TIDAL ; `Ok(None)` si `path` n'en est pas une.
fn command(path: &str, body: &[u8]) -> Result<Option<Command>, &'static str> {
    let command = match path {
        "/api/pause" => Command::SetPaused(parse::<PausedBody>(body)?.paused),
        "/api/next" => Command::Next,
        "/api/previous" => Command::Previous,
        "/api/seek" => {
            let seconds = parse::<SeekBody>(body)?.seconds;
            if !seconds.is_finite() || seconds < 0.0 {
                return Err("body");
            }
            Command::Seek(seconds)
        }
        "/api/volume" => {
            let level = parse::<VolumeBody>(body)?.level;
            Command::Volume(Volume::validate(level).map_err(|_| "body")?)
        }
        "/api/mute" => Command::Muted(parse::<MutedBody>(body)?.muted),
        "/api/shuffle" => Command::Shuffle(parse::<EnabledBody>(body)?.enabled),
        "/api/repeat" => Command::Repeat(parse::<RepeatBody>(body)?.mode),
        "/api/jump" => {
            let b = parse::<JumpBody>(body)?;
            Command::Jump {
                part: b.part,
                index: b.index,
                id: TrackId(b.track_id),
            }
        }
        "/api/remove" => Command::Remove(TrackId(parse::<TrackBody>(body)?.track_id)),
        "/api/play" => Command::Play {
            ids: vec![TrackId(parse::<TrackBody>(body)?.track_id)],
            shuffle: false,
            source: None,
        },
        "/api/enqueue" => {
            let b = parse::<TrackBody>(body)?;
            Command::Enqueue {
                ids: vec![TrackId(b.track_id)],
                next: b.next,
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(command))
}

/// Album ou playlist entiers, lus depuis le début (ou mélangés).
async fn context<R: Remote>(path: &str, body: &[u8], remote: &R) -> Result<Command, Response> {
    let (tracks, source, shuffle) = if path == "/api/play-album" {
        let b = parse::<AlbumBody>(body).map_err(|kind| Response::error(400, kind))?;
        let (title, tracks) = remote
            .album(b.album_id)
            .await
            .map_err(|e| backend_error(&e))?;
        let source = PlaySource::Album {
            id: b.album_id,
            title,
        };
        (tracks, source, b.shuffle)
    } else {
        let b = parse::<PlaylistBody>(body).map_err(|kind| Response::error(400, kind))?;
        if !crate::commands::settings::pins::is_uuid(&b.uuid) {
            return Err(Response::error(400, "body"));
        }
        let (title, tracks) = remote
            .playlist(b.uuid.clone())
            .await
            .map_err(|e| backend_error(&e))?;
        let source = PlaySource::Playlist {
            uuid: b.uuid,
            title,
        };
        (tracks, source, b.shuffle)
    };
    if tracks.is_empty() {
        return Err(Response::error(400, "empty"));
    }
    Ok(Command::Play {
        ids: tracks.iter().take(MAX_CONTEXT).map(|t| t.id).collect(),
        shuffle,
        source: Some(source),
    })
}

fn parse<T: DeserializeOwned>(body: &[u8]) -> Result<T, &'static str> {
    serde_json::from_slice(body).map_err(|_| "body")
}

fn ok() -> Response {
    Response::json(200, &serde_json::json!({ "ok": true }))
}

/// Le détail reste dans le journal : la page n'affiche qu'un message générique.
pub(super) fn backend_error(e: &str) -> Response {
    tracing::warn!("Télécommande : {e}");
    Response::error(502, "backend")
}

#[derive(Deserialize)]
struct PausedBody {
    paused: bool,
}

#[derive(Deserialize)]
struct SeekBody {
    seconds: f64,
}

#[derive(Deserialize)]
struct VolumeBody {
    level: u8,
}

#[derive(Deserialize)]
struct MutedBody {
    muted: bool,
}

#[derive(Deserialize)]
struct EnabledBody {
    enabled: bool,
}

#[derive(Deserialize)]
struct RepeatBody {
    mode: Repeat,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JumpBody {
    part: QueuePart,
    index: usize,
    track_id: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackBody {
    track_id: u64,
    /// `enqueue` : en tête de la file manuelle (« Lire ensuite ») plutôt qu'à la fin.
    #[serde(default)]
    next: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AlbumBody {
    album_id: u64,
    #[serde(default)]
    shuffle: bool,
}

#[derive(Deserialize)]
struct PlaylistBody {
    uuid: String,
    #[serde(default)]
    shuffle: bool,
}

#[derive(Deserialize)]
struct SearchBody {
    query: String,
}

mod handoff;
mod history;
pub(super) use history::PATH as HISTORY_PATH;
#[cfg(test)]
mod tests;
