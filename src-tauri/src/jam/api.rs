//! Cœur de l'API des invités (#25), commun à la Jam locale (`server.rs`) et au relais
//! (`relay.rs`) : code d'accès, limites par adresse, titres et albums proposés, ajouts des
//! invités, état. Les deux transports ne font que livrer des requêtes ici et rendre les
//! réponses.

use std::collections::HashMap;
use std::future::Future;
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Deserialize;
use tokio::sync::Notify;

use super::guard::{Action, Limits, Offered, code_matches};
use super::guests::{Guests, sanitize_name, sanitize_token};
use super::http::Response;
use super::view::{GuestAlbum, GuestState, GuestTrack};
use crate::tidal::models::{AlbumSummary, Track, TrackId};

/// Délai d'une réponse qui attend TIDAL ou le lecteur.
pub const BACKEND_TIMEOUT: Duration = Duration::from_secs(15);
/// Un lien collé (Apple Music, Spotify avec ses paramètres) dépasse souvent 100 caractères.
pub const MAX_QUERY_CHARS: usize = 300;
const MAX_RESULTS: usize = 30;
const MAX_ALBUM_RESULTS: usize = 20;
/// Titres d'un album ajoutés au plus : un coffret ne remplit pas la file (#223).
pub const MAX_ALBUM_TRACKS: usize = 30;
/// Titres de la suite montrés aux invités.
pub const MAX_QUEUE: usize = 20;

/// État du lecteur vu par la Jam.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub now: Option<Track>,
    pub paused: bool,
    /// File manuelle.
    pub manual: Vec<Track>,
    /// Suite du contexte.
    pub upcoming: Vec<Track>,
}

/// Résultats d'une recherche de la Jam.
#[derive(Debug, Clone, Default)]
pub struct Hits {
    pub tracks: Vec<Track>,
    pub albums: Vec<AlbumSummary>,
}

/// Ce que le serveur demande à l'app ; remplacé par un faux dans les tests.
pub trait Backend: Send + Sync + 'static {
    fn search(&self, query: String) -> impl Future<Output = Result<Hits, String>> + Send;
    /// Titres ou albums d'un lien collé (TIDAL, Deezer, Spotify, Apple Music, YouTube,
    /// #229) ; appelé seulement si `crate::foreign::find` reconnaît un lien.
    fn link(&self, text: String) -> impl Future<Output = Result<Hits, String>> + Send;
    /// Titres lisibles d'un album, dans l'ordre (#223).
    fn album_tracks(&self, id: u64) -> impl Future<Output = Result<Vec<Track>, String>> + Send;
    /// Ajoute à la fin de la file manuelle, dans l'ordre donné.
    fn enqueue(&self, ids: Vec<TrackId>) -> impl Future<Output = Result<(), String>> + Send;
    /// Retire de la file manuelle une entrée par identifiant (#223).
    fn remove(&self, ids: Vec<TrackId>) -> impl Future<Output = Result<(), String>> + Send;
    /// Lecture (`false`) ou pause (`true`), demandée par un invité (#222).
    fn set_paused(&self, paused: bool) -> impl Future<Output = Result<(), String>> + Send;
    fn state(&self) -> impl Future<Output = Result<Snapshot, String>> + Send;
    /// Les ajouts des invités ont changé : l'UI de l'hôte affiche les noms dans la file.
    fn guests_changed(&self, guests: &Guests);
}

struct Session {
    code: String,
    limits: Limits,
    offered: Offered,
    offered_albums: Offered,
}

/// État d'une Jam côté hôte, quel que soit le transport.
pub struct Core {
    session: Mutex<Session>,
    guests: Arc<Mutex<Guests>>,
    /// Les invités peuvent mettre en pause et relancer (#222) ; activé par défaut.
    controls: AtomicBool,
    /// Relire l'état sans attendre (ajout d'un invité, nouvelle page, pause).
    pub refresh: Notify,
}

impl Core {
    pub fn new(code: String, guests: Arc<Mutex<Guests>>) -> Self {
        Self {
            session: Mutex::new(Session {
                code,
                limits: Limits::default(),
                offered: Offered::default(),
                offered_albums: Offered::default(),
            }),
            guests,
            controls: AtomicBool::new(true),
            refresh: Notify::new(),
        }
    }

    pub fn controls(&self) -> bool {
        self.controls.load(Ordering::Relaxed)
    }

    /// Choix de l'hôte, poussé aussitôt aux pages ouvertes (le bouton y apparaît ou part).
    pub fn set_controls(&self, enabled: bool) {
        self.controls.store(enabled, Ordering::Relaxed);
        self.refresh.notify_one();
    }

    /// Nom de l'invité de chaque titre ajouté par la Jam, pour l'UI de l'hôte.
    pub fn guest_names(&self) -> HashMap<u64, String> {
        self.guests.lock().map(|g| g.snapshot()).unwrap_or_default()
    }

    /// Adresse bloquée ou code faux (compté comme un essai raté) : la réponse d'erreur.
    pub fn check_code(&self, ip: IpAddr, given: &str) -> Result<(), Response> {
        let Ok(mut s) = self.session.lock() else {
            return Err(Response::error(502, "backend"));
        };
        if s.limits.blocked(ip) {
            return Err(Response::error(429, "blocked"));
        }
        if !code_matches(&s.code, given) {
            s.limits.fail(ip);
            return Err(Response::error(401, "code"));
        }
        Ok(())
    }

    fn allow(&self, ip: IpAddr, action: Action) -> bool {
        self.session
            .lock()
            .map(|mut s| s.limits.allow(ip, action, Instant::now()))
            .unwrap_or(false)
    }

    /// État des invités en JSON, `None` si le lecteur ne répond pas.
    pub async fn state_json<B: Backend>(&self, backend: &B) -> Option<String> {
        let snapshot = tokio::time::timeout(BACKEND_TIMEOUT, backend.state())
            .await
            .ok()?
            .ok()?;
        let guests = self.guests.lock().ok()?;
        let state = GuestState::new(&snapshot, &guests, self.controls());
        Some(serde_json::json!(state).to_string())
    }

    /// Appel `POST /api/<…>` d'un invité, `path` compris (`/api/search`).
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
        match path {
            "/api/join" => Response::json(200, &serde_json::json!({ "ok": true })),
            "/api/state" => match self.state_json(backend).await {
                Some(json) => Response::new(200, "application/json", json),
                None => Response::error(502, "backend"),
            },
            "/api/search" => self.search(body, ip, backend).await,
            "/api/add" => self.add(body, ip, backend).await,
            "/api/add-album" => self.add_album(body, ip, backend).await,
            "/api/remove" => self.remove(body, ip, backend).await,
            "/api/pause" => self.pause(body, ip, backend).await,
            _ => Response::not_found(),
        }
    }

    /// Titres seuls (tableau) pour une page d'avant #223, titres et albums sinon.
    async fn search<B: Backend>(&self, body: &[u8], ip: IpAddr, backend: &B) -> Response {
        let Ok(body) = serde_json::from_slice::<SearchBody>(body) else {
            return Response::error(400, "body");
        };
        let query = body.query.trim();
        if query.is_empty() {
            return if body.albums {
                Response::json(200, &serde_json::json!({ "tracks": [], "albums": [] }))
            } else {
                Response::json(200, &serde_json::json!([]))
            };
        }
        if query.chars().count() > MAX_QUERY_CHARS {
            return Response::error(400, "query");
        }
        if !self.allow(ip, Action::Search) {
            return Response::error(429, "slow");
        }
        // Un lien d'un autre service (#229) : ses titres ou son album, pas une recherche.
        let link = crate::foreign::find(query).is_some();
        let hits = if link {
            backend.link(query.to_owned()).await
        } else {
            backend.search(query.to_owned()).await
        };
        let hits = match hits {
            Ok(hits) => hits,
            Err(e) => return backend_error(&e),
        };
        let tracks: Vec<GuestTrack> = hits
            .tracks
            .iter()
            .take(MAX_RESULTS)
            .map(GuestTrack::from)
            .collect();
        let albums: Vec<GuestAlbum> = if body.albums {
            hits.albums
                .iter()
                .take(MAX_ALBUM_RESULTS)
                .map(GuestAlbum::from)
                .collect()
        } else {
            Vec::new()
        };
        if let Ok(mut s) = self.session.lock() {
            s.offered.note(tracks.iter().map(|t| t.id));
            s.offered_albums.note(albums.iter().map(|a| a.id));
        }
        if body.albums {
            let hits = serde_json::json!({ "tracks": tracks, "albums": albums, "link": link });
            Response::json(200, &hits)
        } else {
            Response::json(200, &serde_json::json!(tracks))
        }
    }

    async fn add<B: Backend>(&self, body: &[u8], ip: IpAddr, backend: &B) -> Response {
        let Ok(body) = serde_json::from_slice::<AddBody>(body) else {
            return Response::error(400, "body");
        };
        let offered = self
            .session
            .lock()
            .map(|s| s.offered.contains(body.track_id))
            .unwrap_or(false);
        if !offered {
            return Response::error(400, "unknown");
        }
        if !self.allow(ip, Action::Add) {
            return Response::error(429, "slow");
        }
        if let Err(e) = backend.enqueue(vec![TrackId(body.track_id)]).await {
            return backend_error(&e);
        }
        let batch = self.note(&[body.track_id], &body.guest, backend);
        Response::json(200, &serde_json::json!({ "ok": true, "batch": batch }))
    }

    /// Les titres d'un album à la fin de la file, en un seul lot (#223).
    async fn add_album<B: Backend>(&self, body: &[u8], ip: IpAddr, backend: &B) -> Response {
        let Ok(body) = serde_json::from_slice::<AddAlbumBody>(body) else {
            return Response::error(400, "body");
        };
        let offered = self
            .session
            .lock()
            .map(|s| s.offered_albums.contains(body.album_id))
            .unwrap_or(false);
        if !offered {
            return Response::error(400, "unknown");
        }
        if !self.allow(ip, Action::Add) {
            return Response::error(429, "slow");
        }
        let tracks = match backend.album_tracks(body.album_id).await {
            Ok(tracks) => tracks,
            Err(e) => return backend_error(&e),
        };
        let ids: Vec<u64> = tracks
            .iter()
            .map(|t| t.id.0)
            .take(MAX_ALBUM_TRACKS)
            .collect();
        if ids.is_empty() {
            return Response::error(400, "empty");
        }
        if let Err(e) = backend
            .enqueue(ids.iter().copied().map(TrackId).collect())
            .await
        {
            return backend_error(&e);
        }
        let batch = self.note(&ids, &body.guest, backend);
        Response::json(
            200,
            &serde_json::json!({ "ok": true, "batch": batch, "count": ids.len() }),
        )
    }

    /// Note un ajout sous le nom et le jeton de l'invité ; renvoie le lot.
    fn note<B: Backend>(&self, ids: &[u64], guest: &GuestFields, backend: &B) -> Option<u32> {
        let name = guest.name.as_deref().and_then(sanitize_name);
        let owner = guest.guest.as_deref().and_then(sanitize_token);
        let batch = {
            let mut guests = self.guests.lock().ok()?;
            let batch = guests.new_batch();
            for id in ids {
                guests.note(*id, batch, name.clone(), owner.clone());
            }
            backend.guests_changed(&guests);
            batch
        };
        self.refresh.notify_one();
        Some(batch)
    }

    /// Retire un lot de l'invité, ou un seul de ses titres (#223). Seul l'auteur du lot,
    /// reconnu par son jeton, peut le faire.
    async fn remove<B: Backend>(&self, body: &[u8], ip: IpAddr, backend: &B) -> Response {
        let Ok(body) = serde_json::from_slice::<RemoveBody>(body) else {
            return Response::error(400, "body");
        };
        let Some(owner) = sanitize_token(&body.guest) else {
            return Response::error(400, "body");
        };
        if !self.allow(ip, Action::Remove) {
            return Response::error(429, "slow");
        }
        let ids = {
            let Ok(mut guests) = self.guests.lock() else {
                return Response::error(502, "backend");
            };
            let ids = guests.take(&owner, body.batch, body.track_id);
            if !ids.is_empty() {
                backend.guests_changed(&guests);
            }
            ids
        };
        if !ids.is_empty() {
            if let Err(e) = backend
                .remove(ids.iter().copied().map(TrackId).collect())
                .await
            {
                return backend_error(&e);
            }
            self.refresh.notify_one();
        }
        Response::json(
            200,
            &serde_json::json!({ "ok": true, "removed": ids.len() }),
        )
    }

    /// Lecture ou pause explicite (pas une bascule : deux invités qui appuient en même
    /// temps obtiennent le même résultat).
    async fn pause<B: Backend>(&self, body: &[u8], ip: IpAddr, backend: &B) -> Response {
        let Ok(body) = serde_json::from_slice::<PauseBody>(body) else {
            return Response::error(400, "body");
        };
        if !self.controls() {
            return Response::error(403, "controls");
        }
        if !self.allow(ip, Action::Control) {
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

#[derive(Deserialize)]
struct SearchBody {
    query: String,
    /// Page qui sait afficher les albums (#223).
    #[serde(default)]
    albums: bool,
}

/// Prénom affiché et jeton du navigateur, facultatifs.
#[derive(Deserialize)]
struct GuestFields {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    guest: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddBody {
    track_id: u64,
    #[serde(flatten)]
    guest: GuestFields,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddAlbumBody {
    album_id: u64,
    #[serde(flatten)]
    guest: GuestFields,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoveBody {
    guest: String,
    batch: u32,
    /// Un seul titre du lot ; tout le lot sinon.
    #[serde(default)]
    track_id: Option<u64>,
}

/// Le détail reste dans le journal de l'hôte : la page n'affiche qu'un message générique.
fn backend_error(e: &str) -> Response {
    tracing::warn!("Jam : {e}");
    Response::error(502, "backend")
}
