//! Minimal HTTP/1.1 of the Jam: one request per connection, bounded head and body.
//!
//! Written by hand like `mpv::serve` (no dependency): the guest page only makes GETs of
//! fixed files and short JSON POSTs.

use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::{Instant, timeout_at};

const MAX_HEAD: usize = 8 * 1024;
const MAX_BODY: usize = 2 * 1024;
/// Body of a route that accepts it (`Site::large_body`): playback handoff (#414).
pub const MAX_LARGE_BODY: usize = 4 * 1024 * 1024;
/// A large body gets `base / 5` more per slice of this size (#60).
const LARGE_SLICE: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Other,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Head {
    pub method: Method,
    /// Path without the query string.
    pub path: String,
    /// Query string, without the `?`.
    pub query: String,
    /// `X-Jam-Code` header.
    pub code: Option<String>,
    pub content_length: usize,
}

#[derive(Debug)]
pub struct Request {
    pub head: Head,
    pub body: Vec<u8>,
}

/// Time a whole request may take, from its first byte (#60): `base` for the head and a
/// small body, as for every request of the Jam and the remote page. A large body (only
/// read once `allow_large` accepted it, code checked) gets `base / 5` more per 256 KiB,
/// up to `4 × base`: with `base` = 5 s, 4 MiB in 20 s (about 1.7 Mbit/s, weak Wi-Fi), the
/// phone's own limit on the local network.
pub fn read_time(base: Duration, content_length: usize) -> Duration {
    if content_length <= MAX_BODY {
        return base;
    }
    let slices = u32::try_from(content_length.div_ceil(LARGE_SLICE)).unwrap_or(u32::MAX);
    base.saturating_add((base / 5).saturating_mul(slices))
        .min(base.saturating_mul(4))
}

/// Reads a whole request within `read_time(base, …)`; `None` if it is unreadable, too
/// large, cut or too slow. A body over 2 KiB is only read if `allow_large` accepts it from
/// the head (path, code), and never beyond `MAX_LARGE_BODY`.
pub async fn read_request<S: AsyncRead + Unpin>(
    stream: &mut S,
    allow_large: impl FnOnce(&Head) -> bool,
    base: Duration,
) -> Option<Request> {
    let start = Instant::now();
    let mut buf = Vec::with_capacity(1024);
    let end = timeout_at(start + base, read_head(stream, &mut buf))
        .await
        .ok()??;
    let head = parse_head(&buf[..end])?;
    if head.content_length > MAX_BODY
        && (head.content_length > MAX_LARGE_BODY || !allow_large(&head))
    {
        return None;
    }
    let mut body = buf.split_off(end);
    let deadline = start + read_time(base, head.content_length);
    timeout_at(deadline, read_body(stream, &mut body, head.content_length))
        .await
        .ok()??;
    body.truncate(head.content_length);
    Some(Request { head, body })
}

/// Reads up to the empty line; returns where the head ends in `buf` (which may already
/// hold the start of the body).
async fn read_head<S: AsyncRead + Unpin>(stream: &mut S, buf: &mut Vec<u8>) -> Option<usize> {
    let mut chunk = [0u8; 1024];
    loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            return Some(i + 4);
        }
        if buf.len() >= MAX_HEAD {
            return None;
        }
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return None,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
}

/// Reads until `body` holds `length` bytes.
async fn read_body<S: AsyncRead + Unpin>(
    stream: &mut S,
    body: &mut Vec<u8>,
    length: usize,
) -> Option<()> {
    let mut chunk = [0u8; 8 * 1024];
    while body.len() < length {
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return None,
            Ok(n) => body.extend_from_slice(&chunk[..n]),
        }
    }
    Some(())
}

/// Request head (request line and fields, up to and including the empty line).
pub fn parse_head(raw: &[u8]) -> Option<Head> {
    let text = std::str::from_utf8(raw).ok()?;
    let mut lines = text.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = match first.next()? {
        "GET" => Method::Get,
        "POST" => Method::Post,
        _ => Method::Other,
    };
    let target = first.next()?;
    if !target.starts_with('/') || !first.next()?.starts_with("HTTP/1.") {
        return None;
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let (path, query) = (path.to_owned(), query.to_owned());
    let mut code = None;
    let mut content_length = 0;
    for line in lines.take_while(|l| !l.is_empty()) {
        let (name, value) = line.split_once(':')?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value.parse().ok()?;
        } else if name.eq_ignore_ascii_case("x-jam-code") {
            code = Some(value.to_owned());
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            // Chunked body: never sent by `fetch` for a body of this size.
            return None;
        }
    }
    Some(Head {
        method,
        path,
        query,
        code,
        content_length,
    })
}

/// Value of a query string parameter, not decoded (alphanumeric codes).
pub fn query_param<'a>(query: &'a str, name: &str) -> Option<&'a str> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
}

pub struct Response {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

/// Policy of the guest page: nothing but its own files and TIDAL's covers.
const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; \
                   img-src https://resources.tidal.com; connect-src 'self'; \
                   base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

impl Response {
    pub fn new(status: u16, content_type: &'static str, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            content_type,
            body: body.into(),
        }
    }

    pub fn json(status: u16, value: &serde_json::Value) -> Self {
        Self::new(status, "application/json", value.to_string())
    }

    pub fn error(status: u16, kind: &str) -> Self {
        Self::json(status, &serde_json::json!({ "error": kind }))
    }

    pub fn not_found() -> Self {
        Self::new(404, "text/plain; charset=utf-8", "")
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n\
             Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\
             Referrer-Policy: no-referrer\r\nContent-Security-Policy: {CSP}\r\n\
             Connection: close\r\n\r\n",
            self.status,
            reason(self.status),
            self.content_type,
            self.body.len()
        )
        .into_bytes();
        out.extend_from_slice(&self.body);
        out
    }
}

pub async fn write_response<S: AsyncWrite + Unpin>(stream: &mut S, response: &Response) {
    let _ = stream.write_all(&response.to_bytes()).await;
    let _ = stream.shutdown().await;
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        429 => "Too Many Requests",
        502 => "Bad Gateway",
        _ => "Error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: Duration = Duration::from_secs(5);

    #[test]
    fn parses_post_with_code_and_length() {
        let head = parse_head(
            b"POST /api/search?x=1 HTTP/1.1\r\nHost: a\r\nx-jam-code: AB12CD\r\nContent-Length: 12\r\n\r\n",
        )
        .unwrap();
        assert_eq!(
            head,
            Head {
                method: Method::Post,
                path: "/api/search".into(),
                query: "x=1".into(),
                code: Some("AB12CD".into()),
                content_length: 12,
            }
        );
    }

    #[test]
    fn rejects_malformed_heads() {
        assert!(parse_head(b"GET\r\n\r\n").is_none());
        assert!(parse_head(b"GET relative HTTP/1.1\r\n\r\n").is_none());
        assert!(parse_head(b"GET / SPDY\r\n\r\n").is_none());
        assert!(parse_head(b"POST / HTTP/1.1\r\nContent-Length: x\r\n\r\n").is_none());
        assert!(parse_head(b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n").is_none());
        assert_eq!(
            parse_head(b"DELETE / HTTP/1.1\r\n\r\n").unwrap().method,
            Method::Other
        );
    }

    #[test]
    fn finds_query_params() {
        assert_eq!(query_param("a=1&code=AB23CD", "code"), Some("AB23CD"));
        assert_eq!(query_param("codex=1", "code"), None);
        assert_eq!(query_param("", "code"), None);
    }

    #[test]
    fn large_bodies_get_more_time() {
        let secs = |length| read_time(BASE, length).as_secs_f64();
        assert_eq!(secs(0), 5.0);
        assert_eq!(secs(MAX_BODY), 5.0);
        // One slice of 256 KiB started: one more second.
        assert_eq!(secs(MAX_BODY + 1), 6.0);
        assert_eq!(secs(1024 * 1024), 9.0);
        // 4 MiB would be 21 s: capped at the phone's 20 s.
        assert_eq!(secs(MAX_LARGE_BODY), 20.0);
        assert_eq!(secs(usize::MAX), 20.0);
    }

    #[tokio::test]
    async fn reads_body_split_across_packets() {
        let (mut client, mut server) = tokio::io::duplex(64);
        tokio::spawn(async move {
            client
                .write_all(b"POST /api/add HTTP/1.1\r\nContent-Length: 14\r\n\r\n{\"trackId\"")
                .await
                .unwrap();
            client.write_all(b":42}").await.unwrap();
        });
        let req = read_request(&mut server, |_| false, BASE).await.unwrap();
        assert_eq!(req.body, b"{\"trackId\":42}");
    }

    #[tokio::test]
    async fn refuses_large_bodies() {
        let (mut client, mut server) = tokio::io::duplex(64);
        tokio::spawn(async move {
            let _ = client
                .write_all(b"POST / HTTP/1.1\r\nContent-Length: 999999\r\n\r\n")
                .await;
        });
        assert!(read_request(&mut server, |_| false, BASE).await.is_none());
    }

    #[tokio::test]
    async fn large_bodies_only_where_allowed() {
        let body = vec![b'x'; 10_000];
        let send = |path: &'static str| {
            let body = body.clone();
            let (mut client, server) = tokio::io::duplex(64 * 1024);
            tokio::spawn(async move {
                let head = format!(
                    "POST {path} HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                );
                let _ = client.write_all(head.as_bytes()).await;
                let _ = client.write_all(&body).await;
            });
            server
        };
        let allow = |head: &Head| head.path == "/api/big";
        let mut server = send("/api/big");
        let req = read_request(&mut server, allow, BASE).await.unwrap();
        assert_eq!(req.body.len(), 10_000);
        let mut server = send("/api/small");
        assert!(read_request(&mut server, allow, BASE).await.is_none());
    }

    #[tokio::test]
    async fn never_beyond_the_large_limit() {
        let (mut client, mut server) = tokio::io::duplex(64);
        tokio::spawn(async move {
            let head = format!(
                "POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
                MAX_LARGE_BODY + 1
            );
            let _ = client.write_all(head.as_bytes()).await;
        });
        assert!(read_request(&mut server, |_| true, BASE).await.is_none());
    }

    /// Sends `length` bytes of body in 4 parts, `pause` apart, after the head.
    fn slow(length: usize, pause: Duration) -> tokio::io::DuplexStream {
        let (mut client, server) = tokio::io::duplex(256 * 1024);
        tokio::spawn(async move {
            let head = format!("POST /api/big HTTP/1.1\r\nContent-Length: {length}\r\n\r\n");
            let _ = client.write_all(head.as_bytes()).await;
            let part = vec![b'x'; length / 4];
            for _ in 0..4 {
                tokio::time::sleep(pause).await;
                if client.write_all(&part).await.is_err() {
                    return;
                }
            }
        });
        server
    }

    /// #60 with short durations (base 500 ms): a 1 MiB body has 900 ms in all.
    #[tokio::test]
    async fn a_slow_large_body_is_read_within_its_own_time() {
        let base = Duration::from_millis(500);
        let length = 1024 * 1024;
        // About 600 ms: past the base, within the body's time.
        let mut server = slow(length, Duration::from_millis(150));
        let req = read_request(&mut server, |_| true, base).await;
        assert_eq!(req.map(|r| r.body.len()), Some(length));
        // About 1.2 s: too slow.
        let mut server = slow(length, Duration::from_millis(300));
        assert!(read_request(&mut server, |_| true, base).await.is_none());
    }

    #[tokio::test]
    async fn a_small_body_only_gets_the_base() {
        let base = Duration::from_millis(300);
        let (mut client, mut server) = tokio::io::duplex(1024);
        tokio::spawn(async move {
            let _ = client
                .write_all(b"POST /api/add HTTP/1.1\r\nContent-Length: 2\r\n\r\n{")
                .await;
            tokio::time::sleep(Duration::from_millis(600)).await;
            let _ = client.write_all(b"}").await;
        });
        assert!(read_request(&mut server, |_| false, base).await.is_none());
    }
}
