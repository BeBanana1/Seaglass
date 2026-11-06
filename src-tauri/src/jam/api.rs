//! Cœur de l'API des invités (#25), commun à la Jam locale (`server.rs`) et au relais
//! (`relay.rs`) : code d'accès, limites par adresse, titres proposés, prénoms, état.
//! Les deux transports ne font que livrer des requêtes ici et rendre les réponses.

use std::future::Future;
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use super::guard::{Action, Limits, Offered, code_matches};
use super::guests::{Guests, sanitize_name};
use super::http::Response;
use crate::tidal::models::{Track, TrackId};

/// Délai d'une réponse qui attend TIDAL ou le lecteur.
pub const BACKEND_TIMEOUT: Duration = Duration::from_secs(15);
pub const MAX_QUERY_CHARS: usize = 100;
const MAX_RESULTS: usize = 30;
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

/// Ce que le serveur demande à l'app ; remplacé par un faux dans les tests.
pub trait Backend: Send + Sync + 'static {
    fn search(&self, query: String) -> impl Future<Output = Result<Vec<Track>, String>> + Send;
    fn enqueue(&self, id: TrackId) -> impl Future<Output = Result<(), String>> + Send;
    /// Lecture (`false`) ou pause (`true`), demandée par un invité (#222).
    fn set_paused(&self, paused: bool) -> impl Future<Output = Result<(), String>> + Send;
    fn state(&self) -> impl Future<Output = Result<Snapshot, String>> + Send;
    /// Un invité a ajouté un titre sous son nom : l'UI de l'hôte l'affiche dans la file.
    fn guests_changed(&self, guests: &Guests);
}

/// Titre tel que la page l'affiche : rien d'autre ne sort de l'app.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestTrack {
    pub id: u64,
    pub title: String,
    pub artists: String,
    pub album: String,
    pub cover: Option<String>,
    pub duration: u32,
    /// Invité qui l'a ajouté à la file manuelle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
}

impl From<&Track> for GuestTrack {
    fn from(t: &Track) -> Self {
        let title = match t.version.as_deref().filter(|v| !v.is_empty()) {
            Some(v) => format!("{} ({v})", t.title),
            None => t.title.clone(),
        };
        Self {
            id: t.id.0,
            title,
            artists: t
                .artists
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            album: t.album.title.clone(),
            cover: t.album.cover_small_url.clone(),
            duration: t.duration,
            by: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestState {
    pub now: Option<GuestTrack>,
    pub paused: bool,
    /// Les invités peuvent mettre en pause et relancer (#222, choix de l'hôte).
    pub controls: bool,
    /// File manuelle (avec le nom de l'invité) puis suite du contexte, `MAX_QUEUE` au plus.
    pub queue: Vec<GuestTrack>,
}

impl GuestState {
    pub fn new(snapshot: &Snapshot, guests: &Guests, controls: bool) -> Self {
        let manual = snapshot.manual.iter().map(|t| GuestTrack {
            by: guests.get(t.id.0).map(str::to_owned),
            ..GuestTrack::from(t)
        });
        Self {
            now: snapshot.now.as_ref().map(GuestTrack::from),
            paused: snapshot.paused,
            controls,
            queue: manual
                .chain(snapshot.upcoming.iter().map(GuestTrack::from))
                .take(MAX_QUEUE)
                .collect(),
        }
    }
}

struct Session {
    code: String,
    limits: Limits,
    offered: Offered,
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
    pub fn guest_names(&self) -> std::collections::HashMap<u64, String> {
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
            "/api/pause" => self.pause(body, ip, backend).await,
            _ => Response::not_found(),
        }
    }

    async fn search<B: Backend>(&self, body: &[u8], ip: IpAddr, backend: &B) -> Response {
        let Ok(body) = serde_json::from_slice::<SearchBody>(body) else {
            return Response::error(400, "body");
        };
        let query = body.query.trim();
        if query.is_empty() {
            return Response::json(200, &serde_json::json!([]));
        }
        if query.chars().count() > MAX_QUERY_CHARS {
            return Response::error(400, "query");
        }
        if !self.allow(ip, Action::Search) {
            return Response::error(429, "slow");
        }
        match backend.search(query.to_owned()).await {
            Ok(tracks) => {
                let tracks: Vec<GuestTrack> = tracks
                    .iter()
                    .take(MAX_RESULTS)
                    .map(GuestTrack::from)
                    .collect();
                if let Ok(mut s) = self.session.lock() {
                    s.offered.note(tracks.iter().map(|t| t.id));
                }
                Response::json(200, &serde_json::json!(tracks))
            }
            Err(e) => backend_error(&e),
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
        if let Err(e) = backend.enqueue(TrackId(body.track_id)).await {
            return backend_error(&e);
        }
        if let Some(name) = body.name.as_deref().and_then(sanitize_name)
            && let Ok(mut guests) = self.guests.lock()
        {
            guests.note(body.track_id, name);
            backend.guests_changed(&guests);
        }
        self.refresh.notify_one();
        Response::json(200, &serde_json::json!({ "ok": true }))
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
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddBody {
    track_id: u64,
    #[serde(default)]
    name: Option<String>,
}

/// Le détail reste dans le journal de l'hôte : la page n'affiche qu'un message générique.
fn backend_error(e: &str) -> Response {
    tracing::warn!("Jam : {e}");
    Response::error(502, "backend")
}
