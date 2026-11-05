//! HTTP/1.1 minimal de la Jam : une requête par connexion, en-tête et corps bornés.
//!
//! Écrit à la main comme `mpv::serve` (aucune dépendance) : la page des invités ne fait
//! que des GET de fichiers fixes et des POST JSON courts.

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const MAX_HEAD: usize = 8 * 1024;
const MAX_BODY: usize = 2 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Other,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Head {
    pub method: Method,
    /// Chemin sans la query string.
    pub path: String,
    /// En-tête `X-Jam-Code`.
    pub code: Option<String>,
    pub content_length: usize,
}

#[derive(Debug)]
pub struct Request {
    pub head: Head,
    pub body: Vec<u8>,
}

/// Lit une requête complète ; `None` si elle est illisible, trop grande ou interrompue.
pub async fn read_request<S: AsyncRead + Unpin>(stream: &mut S) -> Option<Request> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    let end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        if buf.len() >= MAX_HEAD {
            return None;
        }
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return None,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    };
    let head = parse_head(&buf[..end])?;
    if head.content_length > MAX_BODY {
        return None;
    }
    let mut body = buf.split_off(end);
    while body.len() < head.content_length {
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return None,
            Ok(n) => body.extend_from_slice(&chunk[..n]),
        }
    }
    body.truncate(head.content_length);
    Some(Request { head, body })
}

/// En-tête de requête (ligne de requête et champs, jusqu'à la ligne vide comprise).
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
    let path = target.split('?').next()?.to_owned();
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
            // Corps découpé : jamais envoyé par `fetch` pour un corps de cette taille.
            return None;
        }
    }
    Some(Head {
        method,
        path,
        code,
        content_length,
    })
}

pub struct Response {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

/// Politique de la page des invités : rien d'autre que ses propres fichiers et les
/// pochettes de TIDAL.
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
        let req = read_request(&mut server).await.unwrap();
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
        assert!(read_request(&mut server).await.is_none());
    }
}
