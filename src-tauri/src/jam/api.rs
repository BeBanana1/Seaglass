//! Cœur de l'API des invités (#25), commun à la Jam locale (`server.rs`) et au relais
//! (`relay.rs`) : code d'accès, limites par invité et par adresse (#256), titres et albums
//! proposés, ajouts des invités, règles et exclusions de l'hôte (#288), plafond de la Jam
//! dans la file (#256), volume (#449), paroles (#448), vote pour passer (#433), état. Les deux transports ne font que livrer des requêtes ici et
//! rendre les réponses.

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

/// Délai d'une réponse qui attend TIDAL ou le lecteur.
pub const BACKEND_TIMEOUT: Duration = Duration::from_secs(15);
/// Un lien collé (Apple Music, Spotify avec ses paramètres) dépasse souvent 100 caractères.
pub const MAX_QUERY_CHARS: usize = 300;
/// Titres d'un album ajoutés au plus : un coffret ne remplit pas la file (#223).
pub const MAX_ALBUM_TRACKS: usize = 30;
/// Titres de la suite montrés aux invités.
pub const MAX_QUEUE: usize = 20;

/// État du lecteur vu par la Jam.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub now: Option<Track>,
    pub paused: bool,
    /// Position dans le titre en cours, en secondes (paroles synchronisées, #448).
    pub position: f64,
    /// File manuelle.
    pub manual: Vec<Track>,
    /// Suite du contexte.
    pub upcoming: Vec<Track>,
    /// Blind test en cours (#434), titre masqué tant que la manche n'est pas révélée.
    pub blind: Option<BlindTestView>,
    /// Volume de l'hôte, montré aux invités seulement s'ils peuvent le régler (#449).
    pub volume: Volume,
}

impl Snapshot {
    /// Blind test ouvert aux invités de la Jam (#441) ; une partie en solo ne les regarde pas.
    pub fn jam_game(&self) -> Option<&BlindTestView> {
        self.blind.as_ref().filter(|b| b.party.jam)
    }
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
    /// Volume demandé par un invité (#449), déjà ramené au plafond de l'hôte.
    fn set_volume(&self, level: u8) -> impl Future<Output = Result<(), String>> + Send;
    /// Passe le titre en cours : les invités l'ont voté (#433). L'hôte en est prévenu.
    fn skip_by_vote(&self) -> impl Future<Output = Result<(), String>> + Send;
    fn state(&self) -> impl Future<Output = Result<Snapshot, String>> + Send;
    /// Paroles d'un titre (#448), `None` s'il n'en a pas.
    fn lyrics(&self, id: TrackId) -> impl Future<Output = Result<Option<Lyrics>, String>> + Send;
    /// Réponse d'un invité au blind test (#441) : son identifiant de joueur, ou le refus
    /// de la partie.
    fn blind_answer(
        &self,
        key: String,
        name: Option<String>,
        round: usize,
        answer: Answer,
    ) -> impl Future<Output = Result<Result<u32, Refusal>, String>> + Send;
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
    /// Paroles du dernier titre demandé par un invité (#448).
    lyrics: tokio::sync::Mutex<Option<lyrics::CachedLyrics>>,
    /// Point de départ du titre en cours envoyé aux invités (#448, `clock.rs`).
    clock: Mutex<Option<Anchor>>,
    /// Votes pour passer le titre en cours (#433).
    skip_votes: Mutex<skip::SkipVotes>,
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
            lyrics: tokio::sync::Mutex::new(None),
            clock: Mutex::new(None),
            skip_votes: Mutex::new(skip::SkipVotes::default()),
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

    /// Position et paroles connues du titre en cours (#448), sauf pendant un blind test.
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
            "/api/blind-answer" => self.blind_answer(body, ip, &key, backend).await,
            "/api/volume" => self.volume(body, ip, &key, backend).await,
            "/api/skip" => self.skip(body, ip, &key, backend).await,
            "/api/lyrics" => self.lyrics(ip, &key, backend).await,
            _ => Response::not_found(),
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
