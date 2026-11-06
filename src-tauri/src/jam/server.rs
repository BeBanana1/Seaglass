//! Jam sur le réseau local (#25) : page des invités et API, servies en HTTP par l'app.
//!
//! Les invités n'ont que trois actions : chercher un titre (compte de l'hôte), l'ajouter
//! à la fin de la file manuelle, voir le titre en cours et la suite (`api.rs`). Chaque
//! appel de l'API porte le code d'accès dans l'en-tête `X-Jam-Code` : un en-tête
//! personnalisé oblige aussi un autre site à passer par une requête CORS préalable, que ce
//! serveur ne connaît pas (pas d'ajout depuis une page malveillante ouverte sur le
//! téléphone d'un invité).
//!
//! L'état est poussé aux pages ouvertes (Server-Sent Events, `GET /api/events?code=…` :
//! `EventSource` ne pose pas d'en-tête). Tant qu'une page écoute, l'état du lecteur est
//! relu toutes les secondes et aussitôt après un ajout ; il n'est envoyé que s'il change.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Semaphore, watch};

use super::api::{BACKEND_TIMEOUT, Backend, Core};
use super::http::{Method, Request, Response, query_param, read_request, write_response};

const MAX_CONNECTIONS: usize = 32;
/// Pages ouvertes qui suivent l'état, en plus des requêtes ordinaires.
const MAX_STREAMS: usize = 64;
/// Délai pour recevoir la requête, puis pour envoyer la réponse.
const IO_TIMEOUT: Duration = Duration::from_secs(5);
/// Relecture de l'état du lecteur tant qu'une page écoute.
pub(super) const PUSH_INTERVAL: Duration = Duration::from_secs(1);
/// Commentaire envoyé sur un flux silencieux : garde la connexion ouverte (Wi-Fi, proxys).
const KEEPALIVE: Duration = Duration::from_secs(20);

const PAGE: &str = include_str!("page/index.html");
const SCRIPT: &str = include_str!("page/jam.js");
const STYLE: &str = include_str!("page/jam.css");

struct Shared {
    core: Arc<Core>,
    /// Dernier état envoyé (JSON). L'émetteur vit dans `serve` : la Jam arrêtée, il
    /// disparaît et les flux ouverts se terminent.
    updates: watch::Receiver<Arc<str>>,
}

/// Sert la Jam jusqu'à ce que la tâche soit annulée (fin de la Jam).
pub async fn serve<B: Backend>(listener: TcpListener, core: Arc<Core>, backend: Arc<B>) {
    let (tx, rx) = watch::channel(Arc::<str>::from(""));
    let shared = Arc::new(Shared { core, updates: rx });
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
                if let Err(response) = shared.core.check_code(ip, given) {
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
                shared.core.refresh.notify_one();
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
            () = shared.core.refresh.notified() => {}
        }
        // `Shared` garde un récepteur : au-delà, des pages écoutent.
        if tx.receiver_count() <= 1 {
            continue;
        }
        let Some(json) = shared.core.state_json(backend).await else {
            continue;
        };
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
        (Method::Post, path) if path.starts_with("/api/") => {
            let code = req.head.code.as_deref();
            shared.core.api(path, code, &req.body, ip, backend).await
        }
        (Method::Get, _) => Response::not_found(),
        _ => Response::error(405, "method"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jam::api::{MAX_ALBUM_TRACKS, MAX_QUERY_CHARS, MAX_QUEUE};
    use crate::jam::testing::Fake;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

    const CODE: &str = "AB23CD";

    async fn start() -> (SocketAddr, Arc<Fake>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let fake = Arc::new(Fake::default());
        let core = Arc::new(Core::new(CODE.into(), Arc::default()));
        tokio::spawn(serve(listener, core, Arc::clone(&fake)));
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
    async fn guests_pause_only_when_the_host_allows_it() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let fake = Arc::new(Fake::default());
        let core = Arc::new(Core::new(CODE.into(), Arc::default()));
        tokio::spawn(serve(listener, Arc::clone(&core), Arc::clone(&fake)));

        let pause = r#"{"paused":true}"#;
        assert_eq!(call(addr, "POST", "/api/pause", CODE, pause).await.0, 200);
        assert_eq!(*fake.paused.lock().unwrap(), Some(true));
        let (_, body) = call(addr, "POST", "/api/state", CODE, "").await;
        let state: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            (state["paused"].clone(), state["controls"].clone()),
            (true.into(), true.into())
        );
        assert_eq!(call(addr, "POST", "/api/pause", CODE, "{}").await.0, 400);

        core.set_controls(false);
        let (status, body) = call(addr, "POST", "/api/pause", CODE, r#"{"paused":false}"#).await;
        assert_eq!((status, body.as_str()), (403, r#"{"error":"controls"}"#));
        assert_eq!(*fake.paused.lock().unwrap(), Some(true));
        let (_, body) = call(addr, "POST", "/api/state", CODE, "").await;
        let state: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(state["controls"], false);

        // Limite par adresse.
        core.set_controls(true);
        for _ in 1..crate::jam::guard::CONTROLS_PER_MINUTE {
            assert_eq!(call(addr, "POST", "/api/pause", CODE, pause).await.0, 200);
        }
        assert_eq!(call(addr, "POST", "/api/pause", CODE, pause).await.0, 429);
    }

    #[tokio::test]
    async fn search_returns_albums_only_to_pages_that_ask() {
        let (addr, _) = start().await;
        let (_, body) = call(addr, "POST", "/api/search", CODE, r#"{"query":"x"}"#).await;
        let old: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(old.is_array());

        let query = r#"{"query":"x","albums":true}"#;
        let (_, body) = call(addr, "POST", "/api/search", CODE, query).await;
        let hits: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(hits["tracks"][0]["id"], 1);
        assert_eq!(hits["albums"][0]["id"], 40);
        assert_eq!(hits["albums"][0]["artists"], "A");
        assert_eq!(hits["albums"][0]["tracks"], 35);
        assert_eq!(hits["albums"][0]["year"], "2013");
    }

    #[tokio::test]
    async fn albums_are_added_as_one_batch_that_only_its_owner_removes() {
        let (addr, fake) = start().await;
        let (lea, max) = ("a".repeat(32), "b".repeat(32));
        let album = format!(r#"{{"albumId":40,"name":"Léa","guest":"{lea}"}}"#);
        // Pas encore proposé par une recherche.
        assert_eq!(
            call(addr, "POST", "/api/add-album", CODE, &album).await.0,
            400
        );

        let query = r#"{"query":"x","albums":true}"#;
        assert_eq!(call(addr, "POST", "/api/search", CODE, query).await.0, 200);
        let (status, body) = call(addr, "POST", "/api/add-album", CODE, &album).await;
        assert_eq!(status, 200);
        let added: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(added["count"], MAX_ALBUM_TRACKS);
        let batch = added["batch"].as_u64().unwrap();
        assert_eq!(fake.added.lock().unwrap().len(), MAX_ALBUM_TRACKS);

        let (_, body) = call(addr, "POST", "/api/state", CODE, "").await;
        let state: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(state["queue"][0]["id"], 100);
        assert_eq!(state["queue"][0]["by"], "Léa");
        assert_eq!(state["queue"][0]["batch"], batch);

        // Un autre jeton ne retire rien ; sans jeton valable, la requête est refusée.
        let remove =
            |guest: &str, track: &str| format!(r#"{{"guest":"{guest}","batch":{batch}{track}}}"#);
        let (_, body) = call(addr, "POST", "/api/remove", CODE, &remove(&max, "")).await;
        assert_eq!(body, r#"{"ok":true,"removed":0}"#);
        assert_eq!(
            call(addr, "POST", "/api/remove", CODE, &remove("x", ""))
                .await
                .0,
            400
        );

        // Un titre, puis le reste du lot.
        let one = remove(&lea, r#","trackId":101"#);
        let (_, body) = call(addr, "POST", "/api/remove", CODE, &one).await;
        assert_eq!(body, r#"{"ok":true,"removed":1}"#);
        assert!(!fake.added.lock().unwrap().contains(&101));
        let (_, body) = call(addr, "POST", "/api/remove", CODE, &remove(&lea, "")).await;
        assert_eq!(
            body,
            format!(r#"{{"ok":true,"removed":{}}}"#, MAX_ALBUM_TRACKS - 1)
        );
        assert!(fake.added.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn long_queries_are_refused() {
        let (addr, _) = start().await;
        let body = format!(r#"{{"query":"{}"}}"#, "a".repeat(MAX_QUERY_CHARS + 1));
        assert_eq!(call(addr, "POST", "/api/search", CODE, &body).await.0, 400);
    }
}
