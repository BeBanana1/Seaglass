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

use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Notify, Semaphore, watch};

use super::api::{BACKEND_TIMEOUT, Backend, Core};
use super::http::{Head, Method, Request, Response, query_param, read_request, write_response};

const MAX_CONNECTIONS: usize = 32;
/// Pages ouvertes qui suivent l'état, en plus des requêtes ordinaires.
const MAX_STREAMS: usize = 64;
/// Time to receive a request (more for a large body, `http::read_time`), then to send
/// the answer.
const IO_TIMEOUT: Duration = Duration::from_secs(5);
/// Relecture de l'état du lecteur tant qu'une page écoute.
pub(super) const PUSH_INTERVAL: Duration = Duration::from_secs(1);
/// Commentaire envoyé sur un flux silencieux : garde la connexion ouverte (Wi-Fi, proxys).
const KEEPALIVE: Duration = Duration::from_secs(20);

const PAGE: &str = include_str!("page/index.html");
const SCRIPT: &str = include_str!("page/jam.js");
pub(super) const STYLE: &str = include_str!("page/jam.css");

/// Ce qu'un serveur local sert : la page des invités de la Jam, ou celle de la télécommande
/// (#336). Le serveur ne connaît que ce trait : les droits de chaque page restent dans son
/// API, et aucune ne peut appeler les commandes de l'autre.
pub trait Site: Send + Sync + 'static {
    /// Fichier fixe de la page (`GET`), `None` = 404.
    fn file(&self, path: &str) -> Option<Response>;
    /// Adresse bloquée ou code faux (compté comme un essai raté) : la réponse d'erreur.
    fn check_code(&self, ip: IpAddr, given: &str) -> Result<(), Response>;
    /// Appel `POST /api/<…>`, `path` compris, code de l'en-tête `X-Jam-Code`.
    fn api(
        &self,
        path: &str,
        code: Option<&str>,
        body: &[u8],
        ip: IpAddr,
    ) -> impl Future<Output = Response> + Send;
    /// État poussé aux pages ouvertes, `None` si le lecteur ne répond pas.
    fn state_json(&self) -> impl Future<Output = Option<String>> + Send;
    /// Relire l'état sans attendre.
    fn refresh(&self) -> &Notify;
    /// Vrai si `path` accepte un corps de plus de 2 Kio (`http::MAX_LARGE_BODY` au plus) ;
    /// le code est alors vérifié avant de lire le corps.
    fn large_body(&self, _path: &str) -> bool {
        false
    }
}

/// Page des invités de la Jam.
struct JamSite<B> {
    core: Arc<Core>,
    backend: Arc<B>,
}

impl<B: Backend> Site for JamSite<B> {
    fn file(&self, path: &str) -> Option<Response> {
        match path {
            "/" => Some(Response::new(200, "text/html; charset=utf-8", PAGE)),
            "/jam.js" => Some(Response::new(200, "text/javascript; charset=utf-8", SCRIPT)),
            "/jam.css" => Some(Response::new(200, "text/css; charset=utf-8", STYLE)),
            _ => None,
        }
    }

    fn check_code(&self, ip: IpAddr, given: &str) -> Result<(), Response> {
        self.core.check_code(ip, given)
    }

    async fn api(&self, path: &str, code: Option<&str>, body: &[u8], ip: IpAddr) -> Response {
        self.core.api(path, code, body, ip, &*self.backend).await
    }

    async fn state_json(&self) -> Option<String> {
        self.core.state_json(&*self.backend).await
    }

    fn refresh(&self) -> &Notify {
        &self.core.refresh
    }
}

struct Shared<S> {
    site: Arc<S>,
    /// Dernier état envoyé (JSON). L'émetteur vit dans `serve_site` : le serveur arrêté,
    /// il disparaît et les flux ouverts se terminent.
    updates: watch::Receiver<Arc<str>>,
}

/// Sert la Jam jusqu'à ce que la tâche soit annulée (fin de la Jam).
pub async fn serve<B: Backend>(listener: TcpListener, core: Arc<Core>, backend: Arc<B>) {
    serve_site(listener, Arc::new(JamSite { core, backend })).await;
}

/// Sert `site` jusqu'à ce que la tâche soit annulée.
pub async fn serve_site<S: Site>(listener: TcpListener, site: Arc<S>) {
    let (tx, rx) = watch::channel(Arc::<str>::from(""));
    let shared = Arc::new(Shared { site, updates: rx });
    tokio::select! {
        () = accept_loop(listener, Arc::clone(&shared)) => {}
        () = push_loop(tx, &shared) => {}
    }
}

async fn accept_loop<S: Site>(listener: TcpListener, shared: Arc<Shared<S>>) {
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
        let streams = Arc::clone(&streams);
        tokio::spawn(async move {
            let ip = client_ip(addr);
            // Gros corps (transfert de lecture) : seulement là où le site l'accepte, et avec
            // le bon code, vérifié avant de lire.
            let allow_large = |head: &Head| {
                head.method == Method::Post
                    && shared.site.large_body(&head.path)
                    && shared
                        .site
                        .check_code(ip, head.code.as_deref().unwrap_or(""))
                        .is_ok()
            };
            // The head within `IO_TIMEOUT`, a large body with more time (#60).
            let Some(req) = read_request(&mut stream, allow_large, IO_TIMEOUT).await else {
                return;
            };
            if req.head.method == Method::Get && req.head.path == "/api/events" {
                // Un flux dure : il libère la place des requêtes ordinaires.
                drop(permit);
                let given = query_param(&req.head.query, "code").unwrap_or("");
                if let Err(response) = shared.site.check_code(ip, given) {
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
                shared.site.refresh().notify_one();
                stream_events(stream, shared.updates.clone()).await;
                return;
            }
            let response = tokio::time::timeout(BACKEND_TIMEOUT, route(req, ip, &shared))
                .await
                .unwrap_or_else(|_| Response::error(502, "backend"));
            let _ = tokio::time::timeout(IO_TIMEOUT, write_response(&mut stream, &response)).await;
            drop(permit);
        });
    }
}

/// Relit l'état du lecteur pendant qu'au moins une page écoute, et le publie s'il change.
async fn push_loop<S: Site>(tx: watch::Sender<Arc<str>>, shared: &Shared<S>) {
    loop {
        tokio::select! {
            () = tokio::time::sleep(PUSH_INTERVAL) => {}
            () = shared.site.refresh().notified() => {}
        }
        // `Shared` garde un récepteur : au-delà, des pages écoutent.
        if tx.receiver_count() <= 1 {
            continue;
        }
        let Some(json) = shared.site.state_json().await else {
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

async fn route<S: Site>(req: Request, ip: IpAddr, shared: &Shared<S>) -> Response {
    match (req.head.method, req.head.path.as_str()) {
        (Method::Post, path) if path.starts_with("/api/") => {
            let code = req.head.code.as_deref();
            shared.site.api(path, code, &req.body, ip).await
        }
        (Method::Get, path) => shared.site.file(path).unwrap_or_else(Response::not_found),
        _ => Response::error(405, "method"),
    }
}

#[cfg(test)]
mod tests;
