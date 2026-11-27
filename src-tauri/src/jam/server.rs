//! Jam on the local network (#25): guest page and API, served over HTTP by the app.
//!
//! Guests only have three actions: search for a track (host's account), add it at the end
//! of the manual queue, see the current track and what follows (`api.rs`). Every API call
//! carries the access code in the `X-Jam-Code` header: a custom header also forces another
//! site through a CORS preflight request, which this server does not know (no additions
//! from a malicious page open on a guest's phone).
//!
//! The state is pushed to open pages (Server-Sent Events, `GET /api/events?code=…`:
//! `EventSource` sets no header). While a page listens, the player state is read again
//! every second and right after an addition; it is only sent when it changes.

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
/// Open pages following the state, on top of ordinary requests.
const MAX_STREAMS: usize = 64;
/// Time to receive a request (more for a large body, `http::read_time`), then to send
/// the answer.
const IO_TIMEOUT: Duration = Duration::from_secs(5);
/// Rereading of the player state while a page listens.
pub(super) const PUSH_INTERVAL: Duration = Duration::from_secs(1);
/// Comment sent on a silent stream: keeps the connection open (Wi-Fi, proxies).
const KEEPALIVE: Duration = Duration::from_secs(20);

const PAGE: &str = include_str!("page/index.html");
/// `jam.js`, assembled from the page's files by `page/script.rs` (#45).
const SCRIPT: &str = include!("page/script.rs");
pub(super) const STYLE: &str = include_str!("page/jam.css");

/// What a local server serves: the Jam guest page, or the remote control one (#336). The
/// server only knows this trait: each page's rights stay in its API, and neither can call
/// the other's commands.
pub trait Site: Send + Sync + 'static {
    /// Static file of the page (`GET`), `None` = 404.
    fn file(&self, path: &str) -> Option<Response>;
    /// Blocked address or wrong code (counted as a failed attempt): the error answer.
    fn check_code(&self, ip: IpAddr, given: &str) -> Result<(), Response>;
    /// `POST /api/<…>` call, `path` included, code from the `X-Jam-Code` header.
    fn api(
        &self,
        path: &str,
        code: Option<&str>,
        body: &[u8],
        ip: IpAddr,
    ) -> impl Future<Output = Response> + Send;
    /// State pushed to open pages, `None` if the player does not answer.
    fn state_json(&self) -> impl Future<Output = Option<String>> + Send;
    /// Read the state again without waiting.
    fn refresh(&self) -> &Notify;
    /// True if `path` accepts a body over 2 KiB (`http::MAX_LARGE_BODY` at most); the code is
    /// then checked before the body is read.
    fn large_body(&self, _path: &str) -> bool {
        false
    }
}

/// Jam guest page.
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
    /// Last state sent (JSON). The sender lives in `serve_site`: once the server stops, it
    /// goes away and the open streams end.
    updates: watch::Receiver<Arc<str>>,
}

/// Serves the Jam until the task is cancelled (end of the Jam).
pub async fn serve<B: Backend>(listener: TcpListener, core: Arc<Core>, backend: Arc<B>) {
    serve_site(listener, Arc::new(JamSite { core, backend })).await;
}

/// Serves `site` until the task is cancelled.
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
                tracing::warn!("Jam: {e}");
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
            // Large body (playback handoff): only where the site accepts it, and with the
            // right code, checked before reading.
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
                // A stream lasts: it frees the slot of ordinary requests.
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

/// Reads the player state again while at least one page listens, and publishes it if it
/// changes.
async fn push_loop<S: Site>(tx: watch::Sender<Arc<str>>, shared: &Shared<S>) {
    loop {
        tokio::select! {
            () = tokio::time::sleep(PUSH_INTERVAL) => {}
            () = shared.site.refresh().notified() => {}
        }
        // `Shared` keeps one receiver: beyond it, pages are listening.
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

/// `text/event-stream` stream: the current state, then each change.
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

/// An IPv4 address seen as IPv6 (`::ffff:a.b.c.d`) counts as the IPv4 one.
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
