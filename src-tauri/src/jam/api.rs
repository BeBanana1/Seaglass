//! Cœur de l'API des invités (#25), commun à la Jam locale (`server.rs`) et au relais
//! (`relay.rs`) : code d'accès, limites par invité et par adresse (#256), titres et albums
//! proposés, ajouts des invités, règles et exclusions de l'hôte (#288), plafond de la Jam
//! dans la file (#256), état. Les deux transports ne font que livrer des requêtes ici et
//! rendre les réponses.

mod adds;

use std::collections::HashMap;
use std::future::Future;
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Deserialize;
use tokio::sync::Notify;

use super::guard::{Action, Limits, Offered, code_matches};
use super::guests::{Guests, sanitize_token};
use super::http::Response;
use super::members::{GuestInfo, owner_key};
use super::rules::JamRules;
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
    /// Règles de l'hôte (#222, #288), retenues d'une Jam à l'autre.
    rules: Mutex<JamRules>,
    /// L'hôte peut suspendre les ajouts le temps de la Jam (#288).
    adds_open: AtomicBool,
    /// Exclure un invité bloque aussi son adresse (réseau local seulement, #288).
    ban_ips: bool,
    /// Un ajout à la fois : la limite par invité et les doublons se vérifient sur la file
    /// qui suit l'ajout précédent.
    add_turn: tokio::sync::Mutex<()>,
    /// Relire l'état sans attendre (ajout d'un invité, nouvelle page, pause).
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
            refresh: Notify::new(),
        }
    }

    pub fn rules(&self) -> JamRules {
        self.rules.lock().map(|r| *r).unwrap_or_default()
    }

    /// Choix de l'hôte, poussé aussitôt aux pages ouvertes (boutons et messages y changent).
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

    /// Invités qui ont ajouté pendant la Jam, pour l'UI de l'hôte.
    pub fn members(&self) -> Vec<GuestInfo> {
        self.guests
            .lock()
            .map(|g| g.members.list())
            .unwrap_or_default()
    }

    /// Exclut un invité (ses titres en attente sont retirés) ou le réadmet ; faux si
    /// l'invité n'existe pas.
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
            tracing::warn!("Jam : {e}");
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

    /// Nom de l'invité de chaque titre ajouté par la Jam, pour l'UI de l'hôte.
    pub fn guest_names(&self) -> HashMap<u64, String> {
        self.guests.lock().map(|g| g.snapshot()).unwrap_or_default()
    }

    /// Adresse bloquée ou code faux (compté comme un essai raté) : la réponse d'erreur.
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

    /// Action comptée pour l'invité (`key`, `owner_key`) et pour son adresse (#256).
    fn allow(&self, ip: IpAddr, key: &str, action: Action) -> bool {
        self.session
            .lock()
            .map(|mut s| s.limits.allow(ip, key, action, Instant::now()))
            .unwrap_or(false)
    }

    /// État des invités en JSON, `None` si le lecteur ne répond pas.
    pub async fn state_json<B: Backend>(&self, backend: &B) -> Option<String> {
        let snapshot = tokio::time::timeout(BACKEND_TIMEOUT, backend.state())
            .await
            .ok()?
            .ok()?;
        let guests = self.guests.lock().ok()?;
        let state = GuestState::new(&snapshot, &guests, &self.rules(), self.adds_open());
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
        // Jeton du navigateur, envoyé avec toutes les requêtes depuis #288 : un invité exclu
        // est refusé partout, et la page le lui dit.
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
            _ => Response::not_found(),
        }
    }

    /// Titres seuls (tableau) pour une page d'avant #223, titres et albums sinon.
    async fn search<B: Backend>(
        &self,
        body: &[u8],
        ip: IpAddr,
        key: &str,
        backend: &B,
    ) -> Response {
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
        if !self.allow(ip, key, Action::Search) {
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

    /// Lecture ou pause explicite (pas une bascule : deux invités qui appuient en même
    /// temps obtiennent le même résultat).
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

#[derive(Deserialize)]
struct SearchBody {
    query: String,
    /// Page qui sait afficher les albums (#223).
    #[serde(default)]
    albums: bool,
}

/// Jeton du navigateur, lu dans toute requête (#288).
#[derive(Deserialize)]
struct GuestOnly {
    #[serde(default)]
    guest: Option<String>,
}

/// Le détail reste dans le journal de l'hôte : la page n'affiche qu'un message générique.
fn backend_error(e: &str) -> Response {
    tracing::warn!("Jam : {e}");
    Response::error(502, "backend")
}

#[cfg(test)]
mod tests;
