//! Minimal HTTP server on 127.0.0.1 that serves the DASH manifests from memory.
//!
//! ffmpeg (on mpv's side) only reads an MPD through a URL: this avoids any file on disk.
//! Only the latest published manifests are kept (mpv window of 2 entries).
//!
//! An MPD contains the account's signed stream URLs, and `127.0.0.1` can be reached by any
//! process (and any Windows account) on the machine: every path starts with a random
//! 128-bit token drawn at startup, known only to the app and to mpv.

use std::collections::{HashMap, VecDeque};
use std::fmt::Write as _;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use rand::RngCore;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;

use super::{MpvError, Result};

const MAX_ENTRIES: usize = 4;
const MAX_REQUEST_BYTES: usize = 8 * 1024;
/// mpv opens one connection per manifest: past this, a connection is closed at once.
const MAX_CONNECTIONS: usize = 16;
/// A connection that sends nothing (or does not read the reply) is closed after this delay.
const IO_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Default)]
struct Entries {
    map: HashMap<String, Arc<str>>,
    order: VecDeque<String>,
    counter: u64,
}

#[derive(Clone)]
pub struct ManifestServer {
    /// `http://127.0.0.1:<port>/<jeton>`.
    base: String,
    entries: Arc<RwLock<Entries>>,
}

impl ManifestServer {
    pub async fn start() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(MpvError::Serve)?;
        let addr = listener.local_addr().map_err(MpvError::Serve)?;
        let entries: Arc<RwLock<Entries>> = Arc::default();
        let shared = Arc::clone(&entries);
        let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        // With no free slot, `stream` is dropped here: the connection is closed.
                        let Ok(permit) = Arc::clone(&slots).try_acquire_owned() else {
                            tracing::warn!("manifest server: too many connections");
                            continue;
                        };
                        let entries = Arc::clone(&shared);
                        tokio::spawn(async move {
                            let _ = tokio::time::timeout(IO_TIMEOUT, handle(stream, entries)).await;
                            drop(permit);
                        });
                    }
                    Err(e) => tracing::warn!("manifest server: {e}"),
                }
            }
        });
        Ok(Self {
            base: format!("http://{addr}/{}", random_token()),
            entries,
        })
    }

    /// Publishes a manifest and returns the local URL to give to mpv.
    pub fn publish(&self, label: &str, manifest: String) -> String {
        let Ok(mut e) = self.entries.write() else {
            return format!("{}/unavailable", self.base);
        };
        e.counter += 1;
        let url = format!("{}/{label}-{}.mpd", self.base, e.counter);
        let Some(path) = url_path(&url).map(str::to_owned) else {
            return format!("{}/unavailable", self.base);
        };
        e.map.insert(path.clone(), Arc::from(manifest));
        e.order.push_back(path);
        while e.order.len() > MAX_ENTRIES {
            if let Some(old) = e.order.pop_front() {
                e.map.remove(&old);
            }
        }
        url
    }
}

/// 128 random bits in hexadecimal.
pub(super) fn random_token() -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().fold(String::with_capacity(32), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Path of a `http://host:port/path` URL (it starts with `/`).
fn url_path(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("http://")?;
    rest.find('/').map(|i| &rest[i..])
}

async fn handle(mut stream: TcpStream, entries: Arc<RwLock<Entries>>) {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < MAX_REQUEST_BYTES {
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    let body = request_path(&buf).and_then(|path| {
        entries
            .read()
            .ok()
            .and_then(|e| e.map.get(path).map(Arc::clone))
    });
    let response = match &body {
        Some(mpd) => format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/dash+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            mpd.len()
        ),
        None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
    };
    let _ = stream.write_all(response.as_bytes()).await;
    if let Some(mpd) = body {
        let _ = stream.write_all(mpd.as_bytes()).await;
    }
    let _ = stream.shutdown().await;
}

/// Path of a `GET <path> HTTP/1.x` request, without the query string.
fn request_path(request: &[u8]) -> Option<&str> {
    let line = std::str::from_utf8(request).ok()?.lines().next()?;
    let mut parts = line.split(' ');
    if parts.next()? != "GET" {
        return None;
    }
    parts.next()?.split('?').next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_get_path() {
        assert_eq!(
            request_path(b"GET /123-1.mpd?x=1 HTTP/1.1\r\nHost: a\r\n\r\n"),
            Some("/123-1.mpd")
        );
        assert_eq!(request_path(b"POST /a HTTP/1.1\r\n\r\n"), None);
    }

    #[test]
    fn token_is_128_bits_of_hex() {
        let token = random_token();
        assert_eq!(token.len(), 32);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(token, random_token());
    }

    #[tokio::test]
    async fn serves_published_manifest_and_evicts_old_ones() {
        let server = ManifestServer::start().await.unwrap();
        let first = server.publish("t", "<MPD>0</MPD>".into());
        for i in 1..=MAX_ENTRIES {
            server.publish("t", format!("<MPD>{i}</MPD>"));
        }
        let last = server.publish("t", "<MPD>last</MPD>".into());

        assert_eq!(fetch(&last).await, Some("<MPD>last</MPD>".into()));
        assert_eq!(fetch(&first).await, None);
    }

    #[tokio::test]
    async fn path_without_the_right_token_is_not_found() {
        let server = ManifestServer::start().await.unwrap();
        let url = server.publish("42", "<MPD/>".into());
        assert_eq!(fetch(&url).await, Some("<MPD/>".into()));

        let path = url_path(&url).unwrap();
        let (token, file) = path[1..].split_once('/').unwrap();
        assert_eq!(token.len(), 32);
        let origin = url.strip_suffix(path).unwrap();
        // Without a token: the former path format.
        assert_eq!(status(&format!("{origin}/{file}")).await, Some(404));
        // Mauvais jeton.
        let zeros = "0".repeat(32);
        assert_eq!(status(&format!("{origin}/{zeros}/{file}")).await, Some(404));
        // Token of another server (another launch).
        let other = ManifestServer::start().await.unwrap();
        let other_url = other.publish("42", "<MPD/>".into());
        let other_token = url_path(&other_url).unwrap()[1..]
            .split_once('/')
            .unwrap()
            .0;
        let stolen = format!("{origin}/{other_token}/{file}");
        assert_eq!(status(&stolen).await, Some(404));
    }

    #[tokio::test]
    async fn silent_connection_is_closed_after_timeout() {
        let server = ManifestServer::start().await.unwrap();
        let url = server.publish("t", "<MPD/>".into());
        let mut s = TcpStream::connect(origin_addr(&url)).await.unwrap();
        let mut out = Vec::new();
        let read = tokio::time::timeout(IO_TIMEOUT * 2, s.read_to_end(&mut out)).await;
        assert!(matches!(read, Ok(Ok(0))), "connection not closed: {read:?}");
    }

    #[tokio::test]
    async fn connections_beyond_the_limit_are_refused() {
        let server = ManifestServer::start().await.unwrap();
        let url = server.publish("t", "<MPD/>".into());
        let addr = origin_addr(&url);
        let mut idle = Vec::new();
        for _ in 0..MAX_CONNECTIONS {
            idle.push(TcpStream::connect(addr).await.unwrap());
        }
        // Lets the server accept the silent connections.
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(fetch(&url).await, None);
        drop(idle);
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(fetch(&url).await, Some("<MPD/>".into()));
    }

    fn origin_addr(url: &str) -> &str {
        let rest = url.strip_prefix("http://").unwrap();
        &rest[..rest.find('/').unwrap()]
    }

    /// Raw reply, `None` if the connection is closed without a reply.
    async fn request(url: &str) -> Option<String> {
        let path = url_path(url)?;
        let mut s = TcpStream::connect(origin_addr(url)).await.ok()?;
        s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes())
            .await
            .ok()?;
        let mut out = String::new();
        s.read_to_string(&mut out).await.ok()?;
        (!out.is_empty()).then_some(out)
    }

    async fn status(url: &str) -> Option<u16> {
        request(url).await?.get(9..12)?.parse().ok()
    }

    async fn fetch(url: &str) -> Option<String> {
        let out = request(url).await?;
        if !out.starts_with("HTTP/1.1 200") {
            return None;
        }
        out.split("\r\n\r\n").nth(1).map(str::to_owned)
    }
}
