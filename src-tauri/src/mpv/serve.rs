//! Serveur HTTP minimal sur 127.0.0.1 qui sert les manifestes DASH depuis la mémoire.
//!
//! ffmpeg (côté mpv) ne lit un MPD que par une URL : on évite ainsi tout fichier sur
//! disque. Seuls les derniers manifestes publiés sont conservés (fenêtre mpv de 2 entrées).

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::{MpvError, Result};

const MAX_ENTRIES: usize = 4;
const MAX_REQUEST_BYTES: usize = 8 * 1024;

#[derive(Default)]
struct Entries {
    map: HashMap<String, Arc<str>>,
    order: VecDeque<String>,
    counter: u64,
}

#[derive(Clone)]
pub struct ManifestServer {
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
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        tokio::spawn(handle(stream, Arc::clone(&shared)));
                    }
                    Err(e) => tracing::warn!("serveur de manifestes : {e}"),
                }
            }
        });
        Ok(Self {
            base: format!("http://{addr}"),
            entries,
        })
    }

    /// Publie un manifeste et renvoie l'URL locale à donner à mpv.
    pub fn publish(&self, label: &str, manifest: String) -> String {
        let Ok(mut e) = self.entries.write() else {
            return format!("{}/unavailable", self.base);
        };
        e.counter += 1;
        let path = format!("/{label}-{}.mpd", e.counter);
        e.map.insert(path.clone(), Arc::from(manifest));
        e.order.push_back(path.clone());
        while e.order.len() > MAX_ENTRIES {
            if let Some(old) = e.order.pop_front() {
                e.map.remove(&old);
            }
        }
        format!("{}{path}", self.base)
    }
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

/// Chemin d'une requête `GET <chemin> HTTP/1.x`, sans la query string.
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

    async fn fetch(url: &str) -> Option<String> {
        let rest = url.strip_prefix("http://")?;
        let (addr, path) = rest.split_at(rest.find('/')?);
        let mut s = TcpStream::connect(addr).await.ok()?;
        s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes())
            .await
            .ok()?;
        let mut out = String::new();
        s.read_to_string(&mut out).await.ok()?;
        if !out.starts_with("HTTP/1.1 200") {
            return None;
        }
        out.split("\r\n\r\n").nth(1).map(str::to_owned)
    }
}
