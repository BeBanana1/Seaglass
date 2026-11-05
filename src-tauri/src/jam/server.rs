//! Serveur de la Jam (#25) : page des invités et petite API JSON, sur le réseau local.
//!
//! Les invités n'ont que trois actions : chercher un titre (compte de l'hôte), l'ajouter
//! à la fin de la file manuelle, voir le titre en cours et la suite. Chaque appel de l'API
//! porte le code d'accès dans l'en-tête `X-Jam-Code` : un en-tête personnalisé oblige
//! aussi un autre site à passer par une requête CORS préalable, que ce serveur ne connaît
//! pas (pas d'ajout depuis une page malveillante ouverte sur le téléphone d'un invité).

use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

use super::guard::{Action, Limits, Offered, code_matches};
use super::http::{Method, Request, Response, read_request, write_response};
use crate::tidal::models::{Track, TrackId};

const MAX_CONNECTIONS: usize = 32;
/// Délai pour recevoir la requête, puis pour envoyer la réponse.
const IO_TIMEOUT: Duration = Duration::from_secs(5);
/// Délai d'une réponse qui attend TIDAL ou le lecteur.
const BACKEND_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_QUERY_CHARS: usize = 100;
const MAX_RESULTS: usize = 30;
/// Titres de la suite montrés aux invités.
const MAX_QUEUE: usize = 20;

const PAGE: &str = include_str!("page/index.html");
const SCRIPT: &str = include_str!("page/jam.js");
const STYLE: &str = include_str!("page/jam.css");

/// Ce que le serveur demande à l'app ; remplacé par un faux dans les tests.
pub trait Backend: Send + Sync + 'static {
    fn search(&self, query: String) -> impl Future<Output = Result<Vec<Track>, String>> + Send;
    fn enqueue(&self, id: TrackId) -> impl Future<Output = Result<(), String>> + Send;
    fn state(&self) -> impl Future<Output = Result<GuestState, String>> + Send;
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
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestState {
    pub now: Option<GuestTrack>,
    pub paused: bool,
    /// File manuelle puis suite du contexte, `MAX_QUEUE` titres au plus.
    pub queue: Vec<GuestTrack>,
}

impl GuestState {
    pub fn new(now: Option<&Track>, paused: bool, next: impl Iterator<Item = Track>) -> Self {
        Self {
            now: now.map(GuestTrack::from),
            paused,
            queue: next.take(MAX_QUEUE).map(|t| GuestTrack::from(&t)).collect(),
        }
    }
}

struct Session {
    code: String,
    limits: Limits,
    offered: Offered,
}

/// Accepte les connexions jusqu'à ce que la tâche soit annulée (fin de la Jam).
pub async fn serve<B: Backend>(listener: TcpListener, code: String, backend: Arc<B>) {
    let session = Arc::new(Mutex::new(Session {
        code,
        limits: Limits::default(),
        offered: Offered::default(),
    }));
    let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
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
        let session = Arc::clone(&session);
        let backend = Arc::clone(&backend);
        tokio::spawn(async move {
            let Ok(Some(req)) = tokio::time::timeout(IO_TIMEOUT, read_request(&mut stream)).await
            else {
                return;
            };
            let response = tokio::time::timeout(
                BACKEND_TIMEOUT,
                route(req, client_ip(addr), &session, &*backend),
            )
            .await
            .unwrap_or_else(|_| Response::error(502, "backend"));
            let _ = tokio::time::timeout(IO_TIMEOUT, write_response(&mut stream, &response)).await;
            drop(permit);
        });
    }
}

/// Une adresse IPv4 vue en IPv6 (`::ffff:a.b.c.d`) compte comme l'IPv4.
fn client_ip(addr: SocketAddr) -> IpAddr {
    match addr.ip() {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(IpAddr::V6(v6), IpAddr::V4),
        ip => ip,
    }
}

async fn route<B: Backend>(
    req: Request,
    ip: IpAddr,
    session: &Mutex<Session>,
    backend: &B,
) -> Response {
    match (req.head.method, req.head.path.as_str()) {
        (Method::Get, "/") => Response::new(200, "text/html; charset=utf-8", PAGE),
        (Method::Get, "/jam.js") => Response::new(200, "text/javascript; charset=utf-8", SCRIPT),
        (Method::Get, "/jam.css") => Response::new(200, "text/css; charset=utf-8", STYLE),
        (Method::Post, path) if path.starts_with("/api/") => api(req, ip, session, backend).await,
        (Method::Get, _) => Response::not_found(),
        _ => Response::error(405, "method"),
    }
}

#[derive(Deserialize)]
struct SearchBody {
    query: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddBody {
    track_id: u64,
}

async fn api<B: Backend>(
    req: Request,
    ip: IpAddr,
    session: &Mutex<Session>,
    backend: &B,
) -> Response {
    // Verrou jamais tenu pendant un `await`.
    {
        let Ok(mut s) = session.lock() else {
            return Response::error(502, "backend");
        };
        if s.limits.blocked(ip) {
            return Response::error(429, "blocked");
        }
        let given = req.head.code.as_deref().unwrap_or("");
        if !code_matches(&s.code, given) {
            s.limits.fail(ip);
            return Response::error(401, "code");
        }
    }
    match req.head.path.as_str() {
        "/api/join" => Response::json(200, &serde_json::json!({ "ok": true })),
        "/api/state" => match backend.state().await {
            Ok(state) => Response::json(200, &serde_json::json!(state)),
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
            if !allow(session, ip, Action::Search) {
                return Response::error(429, "slow");
            }
            match backend.search(query.to_owned()).await {
                Ok(tracks) => {
                    let tracks: Vec<GuestTrack> = tracks
                        .iter()
                        .take(MAX_RESULTS)
                        .map(GuestTrack::from)
                        .collect();
                    if let Ok(mut s) = session.lock() {
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
            let offered = session
                .lock()
                .map(|s| s.offered.contains(body.track_id))
                .unwrap_or(false);
            if !offered {
                return Response::error(400, "unknown");
            }
            if !allow(session, ip, Action::Add) {
                return Response::error(429, "slow");
            }
            match backend.enqueue(TrackId(body.track_id)).await {
                Ok(()) => Response::json(200, &serde_json::json!({ "ok": true })),
                Err(e) => backend_error(&e),
            }
        }
        _ => Response::not_found(),
    }
}

fn allow(session: &Mutex<Session>, ip: IpAddr, action: Action) -> bool {
    session
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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    const CODE: &str = "AB23CD";

    #[derive(Default)]
    struct Fake {
        added: Mutex<Vec<u64>>,
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
        async fn state(&self) -> Result<GuestState, String> {
            Ok(GuestState::new(Some(&track(7)), false, (8..50).map(track)))
        }
    }

    async fn start() -> (SocketAddr, Arc<Fake>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let fake = Arc::new(Fake::default());
        tokio::spawn(serve(listener, CODE.into(), Arc::clone(&fake)));
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
        // Bloqué : même le bon code est refusé.
        assert_eq!(call(addr, "POST", "/api/join", CODE, "").await.0, 429);
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

        assert_eq!(call(addr, "POST", "/api/add", CODE, add).await.0, 200);
        assert_eq!(*fake.added.lock().unwrap(), vec![2]);
        assert_eq!(call(addr, "POST", "/api/add", CODE, "{").await.0, 400);
    }

    #[tokio::test]
    async fn state_is_trimmed() {
        let (addr, _) = start().await;
        let (status, body) = call(addr, "POST", "/api/state", CODE, "").await;
        assert_eq!(status, 200);
        let state: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(state["now"]["id"], 7);
        assert_eq!(state["queue"].as_array().unwrap().len(), MAX_QUEUE);
    }

    #[tokio::test]
    async fn long_queries_are_refused() {
        let (addr, _) = start().await;
        let body = format!(r#"{{"query":"{}"}}"#, "a".repeat(MAX_QUERY_CHARS + 1));
        assert_eq!(call(addr, "POST", "/api/search", CODE, &body).await.0, 400);
    }
}
