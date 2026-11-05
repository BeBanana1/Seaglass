//! Serveur de la Jam (#25) : page des invités et petite API JSON, sur le réseau local.
//!
//! Les invités n'ont que trois actions : chercher un titre (compte de l'hôte), l'ajouter
//! à la fin de la file manuelle, voir le titre en cours et la suite. Chaque appel de l'API
//! porte le code d'accès dans l'en-tête `X-Jam-Code` : un en-tête personnalisé oblige
//! aussi un autre site à passer par une requête CORS préalable, que ce serveur ne connaît
//! pas (pas d'ajout depuis une page malveillante ouverte sur le téléphone d'un invité).
//!
//! L'état est poussé aux pages ouvertes (Server-Sent Events, `GET /api/events?code=…` :
//! `EventSource` ne pose pas d'en-tête). Tant qu'une page écoute, l'état du lecteur est
//! relu toutes les secondes et aussitôt après un ajout ; il n'est envoyé que s'il change.

use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Notify, Semaphore, watch};

use super::guard::{Action, Limits, Offered, code_matches};
use super::guests::{Guests, sanitize_name};
use super::http::{Method, Request, Response, query_param, read_request, write_response};
use crate::tidal::models::{Track, TrackId};

const MAX_CONNECTIONS: usize = 32;
/// Pages ouvertes qui suivent l'état, en plus des requêtes ordinaires.
const MAX_STREAMS: usize = 64;
/// Délai pour recevoir la requête, puis pour envoyer la réponse.
const IO_TIMEOUT: Duration = Duration::from_secs(5);
/// Délai d'une réponse qui attend TIDAL ou le lecteur.
const BACKEND_TIMEOUT: Duration = Duration::from_secs(15);
/// Relecture de l'état du lecteur tant qu'une page écoute.
const PUSH_INTERVAL: Duration = Duration::from_secs(1);
/// Commentaire envoyé sur un flux silencieux : garde la connexion ouverte (Wi-Fi, proxys).
const KEEPALIVE: Duration = Duration::from_secs(20);
const MAX_QUERY_CHARS: usize = 100;
const MAX_RESULTS: usize = 30;
/// Titres de la suite montrés aux invités.
const MAX_QUEUE: usize = 20;

const PAGE: &str = include_str!("page/index.html");
const SCRIPT: &str = include_str!("page/jam.js");
const STYLE: &str = include_str!("page/jam.css");

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
    /// File manuelle (avec le nom de l'invité) puis suite du contexte, `MAX_QUEUE` au plus.
    pub queue: Vec<GuestTrack>,
}

impl GuestState {
    pub fn new(snapshot: &Snapshot, guests: &Guests) -> Self {
        let manual = snapshot.manual.iter().map(|t| GuestTrack {
            by: guests.get(t.id.0).map(str::to_owned),
            ..GuestTrack::from(t)
        });
        Self {
            now: snapshot.now.as_ref().map(GuestTrack::from),
            paused: snapshot.paused,
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

struct Shared {
    session: Mutex<Session>,
    guests: Arc<Mutex<Guests>>,
    /// Dernier état envoyé (JSON). L'émetteur vit dans `serve` : la Jam arrêtée, il
    /// disparaît et les flux ouverts se terminent.
    updates: watch::Receiver<Arc<str>>,
    /// Relire l'état sans attendre (ajout d'un invité, nouvelle page).
    refresh: Notify,
}

/// Sert la Jam jusqu'à ce que la tâche soit annulée (fin de la Jam).
pub async fn serve<B: Backend>(
    listener: TcpListener,
    code: String,
    guests: Arc<Mutex<Guests>>,
    backend: Arc<B>,
) {
    let (tx, rx) = watch::channel(Arc::<str>::from(""));
    let shared = Arc::new(Shared {
        session: Mutex::new(Session {
            code,
            limits: Limits::default(),
            offered: Offered::default(),
        }),
        guests,
        updates: rx,
        refresh: Notify::new(),
    });
    tokio::select! {
        () = accept_loop(listener, Arc::clone(&shared), Arc::clone(&backend)) => {}
        () = push_loop(tx, &shared, &*backend) => {}
    }
}

async fn accept_loop<B: Backend>(listener: TcpListener, shared: Arc<Shared>, backend: Arc<B>) {
    let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    let streams = Arc::new(Semaphore::new(MAX_STREAMS));
    loop {
        let (mut stream, addr) = match listener.accept().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Jam : {e}");
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        let Ok(permit) = Arc::clone(&slots).try_acquire_owned() else {
            continue;
        };
        let shared = Arc::clone(&shared);
        let backend = Arc::clone(&backend);
        let streams = Arc::clone(&streams);
        tokio::spawn(async move {
            let Ok(Some(req)) = tokio::time::timeout(IO_TIMEOUT, read_request(&mut stream)).await
            else {
                return;
            };
            let ip = client_ip(addr);
            if req.head.method == Method::Get && req.head.path == "/api/events" {
                // Un flux dure : il libère la place des requêtes ordinaires.
                drop(permit);
                let given = query_param(&req.head.query, "code").unwrap_or("");
                if let Err(response) = check_code(&shared, ip, given) {
                    let _ =
                        tokio::time::timeout(IO_TIMEOUT, write_response(&mut stream, &response))
                            .await;
                    return;
                }
                let Ok(_stream_permit) = streams.try_acquire_owned() else {
                    let response = Response::error(429, "slow");
                    let _ =
                        tokio::time::timeout(IO_TIMEOUT, write_response(&mut stream, &response))
                            .await;
                    return;
                };
                shared.refresh.notify_one();
                stream_events(stream, shared.updates.clone()).await;
                return;
            }
            let response =
                tokio::time::timeout(BACKEND_TIMEOUT, route(req, ip, &shared, &*backend))
                    .await
                    .unwrap_or_else(|_| Response::error(502, "backend"));
            let _ = tokio::time::timeout(IO_TIMEOUT, write_response(&mut stream, &response)).await;
            drop(permit);
        });
    }
}

/// Relit l'état du lecteur pendant qu'au moins une page écoute, et le publie s'il change.
async fn push_loop<B: Backend>(tx: watch::Sender<Arc<str>>, shared: &Shared, backend: &B) {
    loop {
        tokio::select! {
            () = tokio::time::sleep(PUSH_INTERVAL) => {}
            () = shared.refresh.notified() => {}
        }
        // `Shared` garde un récepteur : au-delà, des pages écoutent.
        if tx.receiver_count() <= 1 {
            continue;
        }
        let Ok(Ok(snapshot)) = tokio::time::timeout(BACKEND_TIMEOUT, backend.state()).await else {
            continue;
        };
        let state = match shared.guests.lock() {
            Ok(guests) => GuestState::new(&snapshot, &guests),
            Err(_) => continue,
        };
        let json = serde_json::json!(state).to_string();
        tx.send_if_modified(|current| {
            if **current == *json {
                return false;
            }
            *current = Arc::from(json);
            true
        });
    }
}

/// Flux `text/event-stream` : l'état courant, puis chaque changement.
async fn stream_events(mut stream: TcpStream, mut updates: watch::Receiver<Arc<str>>) {
    let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\
                Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\
                Connection: keep-alive\r\n\r\nretry: 3000\n\n";
    if !send(&mut stream, head).await {
        return;
    }
    updates.mark_changed();
    loop {
        let message = tokio::select! {
            changed = updates.changed() => {
                if changed.is_err() {
                    break;
                }
                let data = Arc::clone(&updates.borrow_and_update());
                if data.is_empty() {
                    continue;
                }
                format!("data: {data}\n\n")
            }
            () = tokio::time::sleep(KEEPALIVE) => ": ping\n\n".to_owned(),
        };
        if !send(&mut stream, &message).await {
            break;
        }
    }
    let _ = stream.shutdown().await;
}

async fn send(stream: &mut TcpStream, text: &str) -> bool {
    matches!(
        tokio::time::timeout(IO_TIMEOUT, stream.write_all(text.as_bytes())).await,
        Ok(Ok(()))
    )
}

/// Une adresse IPv4 vue en IPv6 (`::ffff:a.b.c.d`) compte comme l'IPv4.
fn client_ip(addr: SocketAddr) -> IpAddr {
    match addr.ip() {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(IpAddr::V6(v6), IpAddr::V4),
        ip => ip,
    }
}

async fn route<B: Backend>(req: Request, ip: IpAddr, shared: &Shared, backend: &B) -> Response {
    match (req.head.method, req.head.path.as_str()) {
        (Method::Get, "/") => Response::new(200, "text/html; charset=utf-8", PAGE),
        (Method::Get, "/jam.js") => Response::new(200, "text/javascript; charset=utf-8", SCRIPT),
        (Method::Get, "/jam.css") => Response::new(200, "text/css; charset=utf-8", STYLE),
        (Method::Post, path) if path.starts_with("/api/") => api(req, ip, shared, backend).await,
        (Method::Get, _) => Response::not_found(),
        _ => Response::error(405, "method"),
    }
}

/// Adresse bloquée ou code faux (compté comme un essai raté) : la réponse d'erreur.
fn check_code(shared: &Shared, ip: IpAddr, given: &str) -> Result<(), Response> {
    let Ok(mut s) = shared.session.lock() else {
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

async fn api<B: Backend>(req: Request, ip: IpAddr, shared: &Shared, backend: &B) -> Response {
    if let Err(response) = check_code(shared, ip, req.head.code.as_deref().unwrap_or("")) {
        return response;
    }
    match req.head.path.as_str() {
        "/api/join" => Response::json(200, &serde_json::json!({ "ok": true })),
        "/api/state" => match backend.state().await {
            Ok(snapshot) => match shared.guests.lock() {
                Ok(guests) => {
                    Response::json(200, &serde_json::json!(GuestState::new(&snapshot, &guests)))
                }
                Err(_) => Response::error(502, "backend"),
            },
            Err(e) => backend_error(&e),
        },
        "/api/search" => {
            let Ok(body) = serde_json::from_slice::<SearchBody>(&req.body) else {
                return Response::error(400, "body");
            };
            let query = body.query.trim();
            if query.is_empty() {
                return Response::json(200, &serde_json::json!([]));
            }
            if query.chars().count() > MAX_QUERY_CHARS {
                return Response::error(400, "query");
            }
            if !allow(shared, ip, Action::Search) {
                return Response::error(429, "slow");
            }
            match backend.search(query.to_owned()).await {
                Ok(tracks) => {
                    let tracks: Vec<GuestTrack> = tracks
                        .iter()
                        .take(MAX_RESULTS)
                        .map(GuestTrack::from)
                        .collect();
                    if let Ok(mut s) = shared.session.lock() {
                        s.offered.note(tracks.iter().map(|t| t.id));
                    }
                    Response::json(200, &serde_json::json!(tracks))
                }
                Err(e) => backend_error(&e),
            }
        }
        "/api/add" => {
            let Ok(body) = serde_json::from_slice::<AddBody>(&req.body) else {
                return Response::error(400, "body");
            };
            let offered = shared
                .session
                .lock()
                .map(|s| s.offered.contains(body.track_id))
                .unwrap_or(false);
            if !offered {
                return Response::error(400, "unknown");
            }
            if !allow(shared, ip, Action::Add) {
                return Response::error(429, "slow");
            }
            if let Err(e) = backend.enqueue(TrackId(body.track_id)).await {
                return backend_error(&e);
            }
            if let Some(name) = body.name.as_deref().and_then(sanitize_name)
                && let Ok(mut guests) = shared.guests.lock()
            {
                guests.note(body.track_id, name);
                backend.guests_changed(&guests);
            }
            shared.refresh.notify_one();
            Response::json(200, &serde_json::json!({ "ok": true }))
        }
        _ => Response::not_found(),
    }
}

fn allow(shared: &Shared, ip: IpAddr, action: Action) -> bool {
    shared
        .session
        .lock()
        .map(|mut s| s.limits.allow(ip, action, Instant::now()))
        .unwrap_or(false)
}

/// Le détail reste dans le journal de l'hôte : la page n'affiche qu'un message générique.
fn backend_error(e: &str) -> Response {
    tracing::warn!("Jam : {e}");
    Response::error(502, "backend")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::{AlbumRef, ArtistRef};
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

    const CODE: &str = "AB23CD";

    #[derive(Default)]
    struct Fake {
        added: Mutex<Vec<u64>>,
        named: Mutex<Vec<(u64, String)>>,
    }

    fn track(id: u64) -> Track {
        Track {
            id: TrackId(id),
            title: format!("Titre {id}"),
            version: (id == 2).then(|| "Live".into()),
            duration: 200,
            artists: vec![
                ArtistRef {
                    id: 1,
                    name: "A".into(),
                },
                ArtistRef {
                    id: 2,
                    name: "B".into(),
                },
            ],
            album: AlbumRef {
                id: 9,
                title: "Album".into(),
                cover_url: None,
                cover_small_url: Some("https://resources.tidal.com/x.jpg".into()),
                cover_large_url: None,
                color: None,
            },
            explicit: false,
            hires: false,
            isrc: None,
        }
    }

    impl Backend for Fake {
        async fn search(&self, _query: String) -> Result<Vec<Track>, String> {
            Ok(vec![track(1), track(2)])
        }
        async fn enqueue(&self, id: TrackId) -> Result<(), String> {
            self.added.lock().unwrap().push(id.0);
            Ok(())
        }
        async fn state(&self) -> Result<Snapshot, String> {
            let added = self.added.lock().unwrap();
            Ok(Snapshot {
                now: Some(track(7)),
                paused: false,
                manual: added.iter().map(|id| track(*id)).collect(),
                upcoming: (8..50).map(track).collect(),
            })
        }
        fn guests_changed(&self, guests: &Guests) {
            let mut named: Vec<_> = guests.snapshot().into_iter().collect();
            named.sort();
            *self.named.lock().unwrap() = named;
        }
    }

    async fn start() -> (SocketAddr, Arc<Fake>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let fake = Arc::new(Fake::default());
        tokio::spawn(serve(
            listener,
            CODE.into(),
            Arc::default(),
            Arc::clone(&fake),
        ));
        (addr, fake)
    }

    async fn call(
        addr: SocketAddr,
        method: &str,
        path: &str,
        code: &str,
        body: &str,
    ) -> (u16, String) {
        let mut s = TcpStream::connect(addr).await.unwrap();
        let req = format!(
            "{method} {path} HTTP/1.1\r\nHost: x\r\nX-Jam-Code: {code}\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        s.write_all(req.as_bytes()).await.unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).await.unwrap();
        let status = out[9..12].parse().unwrap();
        let body = out.split_once("\r\n\r\n").unwrap().1.to_owned();
        (status, body)
    }

    async fn search(addr: SocketAddr) {
        let (status, _) = call(addr, "POST", "/api/search", CODE, r#"{"query":"x"}"#).await;
        assert_eq!(status, 200);
    }

    #[tokio::test]
    async fn serves_the_page_without_code() {
        let (addr, _) = start().await;
        let (status, body) = call(addr, "GET", "/", "", "").await;
        assert_eq!(status, 200);
        assert!(body.contains("jam.js"));
        assert_eq!(call(addr, "GET", "/jam.js", "", "").await.0, 200);
        assert_eq!(call(addr, "GET", "/secret", "", "").await.0, 404);
    }

    #[tokio::test]
    async fn api_requires_the_code_and_blocks_guessing() {
        let (addr, _) = start().await;
        assert_eq!(call(addr, "POST", "/api/join", "ab2-3cd", "").await.0, 200);
        for _ in 0..crate::jam::guard::MAX_FAILURES {
            assert_eq!(call(addr, "POST", "/api/join", "ZZZZZZ", "").await.0, 401);
        }
        // Bloqué : même le bon code est refusé, flux compris.
        assert_eq!(call(addr, "POST", "/api/join", CODE, "").await.0, 429);
        let events = format!("/api/events?code={CODE}");
        assert_eq!(call(addr, "GET", &events, "", "").await.0, 429);
    }

    #[tokio::test]
    async fn only_searched_tracks_can_be_added() {
        let (addr, fake) = start().await;
        let add = r#"{"trackId":2}"#;
        let (status, body) = call(addr, "POST", "/api/add", CODE, add).await;
        assert_eq!((status, body.as_str()), (400, r#"{"error":"unknown"}"#));

        let (status, body) = call(addr, "POST", "/api/search", CODE, r#"{"query":" x "}"#).await;
        assert_eq!(status, 200);
        let results: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(results[1]["title"], "Titre 2 (Live)");
        assert_eq!(results[1]["artists"], "A, B");
        assert!(results[0].get("isrc").is_none());
        assert!(results[0].get("by").is_none());

        assert_eq!(call(addr, "POST", "/api/add", CODE, add).await.0, 200);
        assert_eq!(*fake.added.lock().unwrap(), vec![2]);
        assert_eq!(call(addr, "POST", "/api/add", CODE, "{").await.0, 400);
    }

    #[tokio::test]
    async fn guest_names_reach_the_host_and_the_queue() {
        let (addr, fake) = start().await;
        search(addr).await;
        let add = r#"{"trackId":1,"name":"  Léa \n"}"#;
        assert_eq!(call(addr, "POST", "/api/add", CODE, add).await.0, 200);
        assert_eq!(*fake.named.lock().unwrap(), vec![(1, "Léa".to_owned())]);
        // Sans nom : ajouté, mais anonyme.
        assert_eq!(
            call(addr, "POST", "/api/add", CODE, r#"{"trackId":2}"#)
                .await
                .0,
            200
        );

        let (_, body) = call(addr, "POST", "/api/state", CODE, "").await;
        let state: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(state["now"]["id"], 7);
        assert_eq!(state["queue"][0]["by"], "Léa");
        assert!(state["queue"][1].get("by").is_none());
        assert_eq!(state["queue"].as_array().unwrap().len(), MAX_QUEUE);
    }

    #[tokio::test]
    async fn events_push_the_state_and_its_changes() {
        let (addr, _) = start().await;
        let mut s = TcpStream::connect(addr).await.unwrap();
        let req = format!("GET /api/events?code={CODE} HTTP/1.1\r\nHost: x\r\n\r\n");
        s.write_all(req.as_bytes()).await.unwrap();
        let mut lines = BufReader::new(s).lines();
        let mut next_data = async || loop {
            let line = tokio::time::timeout(Duration::from_secs(5), lines.next_line())
                .await
                .expect("aucun événement")
                .unwrap()
                .unwrap();
            if let Some(data) = line.strip_prefix("data: ") {
                return serde_json::from_str::<serde_json::Value>(data).unwrap();
            }
        };
        let first = next_data().await;
        assert_eq!(first["now"]["id"], 7);
        assert_eq!(first["queue"][0]["id"], 8);

        search(addr).await;
        let add = r#"{"trackId":1,"name":"Max"}"#;
        assert_eq!(call(addr, "POST", "/api/add", CODE, add).await.0, 200);
        let second = next_data().await;
        assert_eq!(second["queue"][0]["id"], 1);
        assert_eq!(second["queue"][0]["by"], "Max");
    }

    #[tokio::test]
    async fn events_need_the_code() {
        let (addr, _) = start().await;
        assert_eq!(
            call(addr, "GET", "/api/events?code=ZZZZZZ", "", "").await.0,
            401
        );
    }

    #[tokio::test]
    async fn long_queries_are_refused() {
        let (addr, _) = start().await;
        let body = format!(r#"{{"query":"{}"}}"#, "a".repeat(MAX_QUERY_CHARS + 1));
        assert_eq!(call(addr, "POST", "/api/search", CODE, &body).await.0, 400);
    }
}
