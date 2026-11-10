//! Relais de bout en bout : un faux hôte en WebSocket, des invités en HTTP.

use std::net::SocketAddr;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use seaglass_relay::protocol::{FromHost, Resume, ToHost};
use seaglass_relay::{Config, serve};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

const KEY: &str = "cle-de-test";
const CODE: &str = "AB23CD";

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn start(config: impl FnOnce(&mut Config)) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let mut cfg = Config::new("https://jam.example/", KEY);
    config(&mut cfg);
    tokio::spawn(serve(listener, cfg));
    addr
}

async fn connect_host(addr: SocketAddr, key: &str) -> Result<Socket, String> {
    let mut request = format!("ws://{addr}/host").into_client_request().unwrap();
    request
        .headers_mut()
        .insert("authorization", format!("Bearer {key}").parse().unwrap());
    tokio_tungstenite::connect_async(request)
        .await
        .map(|(socket, _)| socket)
        .map_err(|e| e.to_string())
}

async fn send(socket: &mut Socket, message: &FromHost) {
    let text = serde_json::to_string(message).unwrap();
    socket.send(Message::Text(text.into())).await.unwrap();
}

async fn receive(socket: &mut Socket) -> ToHost {
    loop {
        let message = tokio::time::timeout(Duration::from_secs(5), socket.next())
            .await
            .expect("rien reçu du relais")
            .unwrap()
            .unwrap();
        if let Message::Text(text) = message {
            return serde_json::from_str(&text).unwrap();
        }
    }
}

/// Hôte branché : renvoie la connexion, l'identifiant et le secret de la Jam.
async fn host(addr: SocketAddr, resume: Option<Resume>) -> (Socket, String, String, bool) {
    let mut socket = connect_host(addr, KEY).await.unwrap();
    send(
        &mut socket,
        &FromHost::Hello {
            code: CODE.into(),
            resume,
        },
    )
    .await;
    match receive(&mut socket).await {
        ToHost::Ready {
            id,
            secret,
            url,
            resumed,
        } => {
            assert_eq!(url, format!("https://jam.example/j/{id}/"));
            (socket, id, secret, resumed)
        }
        other => panic!("attendu ready, reçu {other:?}"),
    }
}

/// Répond à chaque requête par le chemin, le client et le code reçus.
fn echo(mut socket: Socket) -> tokio::task::JoinHandle<Socket> {
    tokio::spawn(async move {
        while let Some(Ok(message)) = socket.next().await {
            let Message::Text(text) = message else {
                continue;
            };
            let Ok(ToHost::Request {
                rid,
                client,
                path,
                code,
                body,
            }) = serde_json::from_str(&text)
            else {
                break;
            };
            let echoed =
                serde_json::json!({ "path": path, "client": client, "code": code, "body": body });
            let reply = FromHost::Response {
                rid,
                status: 200,
                body: echoed.to_string(),
            };
            let text = serde_json::to_string(&reply).unwrap();
            if socket.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
        socket
    })
}

async fn http(
    addr: SocketAddr,
    method: &str,
    path: &str,
    code: &str,
    body: &str,
) -> (u16, String, String) {
    let mut s = TcpStream::connect(addr).await.unwrap();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\nX-Jam-Code: {code}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    s.write_all(request.as_bytes()).await.unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    let (head, body) = out.split_once("\r\n\r\n").unwrap();
    (
        head[9..12].parse().unwrap(),
        head.to_owned(),
        body.to_owned(),
    )
}

#[tokio::test]
async fn hosts_need_the_build_key() {
    let addr = start(|_| {}).await;
    let refused = connect_host(addr, "mauvaise").await.unwrap_err();
    assert!(refused.contains("401"), "{refused}");
}

#[tokio::test]
async fn guests_reach_the_host_through_the_relay() {
    let addr = start(|_| {}).await;
    let (socket, id, _, resumed) = host(addr, None).await;
    assert!(!resumed);
    let _host = echo(socket);

    let (status, head, page) = http(addr, "GET", &format!("/j/{id}/"), "", "").await;
    assert_eq!(status, 200);
    assert!(page.contains("jam.js"));
    assert!(
        head.to_ascii_lowercase()
            .contains("content-security-policy")
    );
    assert_eq!(http(addr, "GET", &format!("/j/{id}"), "", "").await.0, 308);
    assert_eq!(http(addr, "GET", "/j/inconnue/", "", "").await.0, 404);

    let path = format!("/j/{id}/api/search");
    let (status, _, body) = http(addr, "POST", &path, CODE, r#"{"query":"x"}"#).await;
    assert_eq!(status, 200);
    let echoed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(echoed["path"], "/api/search");
    assert_eq!(echoed["client"], "127.0.0.1");
    assert_eq!(echoed["code"], CODE);
    assert_eq!(echoed["body"], r#"{"query":"x"}"#);
}

#[tokio::test]
async fn state_is_pushed_to_open_pages() {
    let addr = start(|_| {}).await;
    let (mut socket, id, _, _) = host(addr, None).await;

    let wrong = format!("/j/{id}/api/events?code=ZZZZZZ");
    assert_eq!(http(addr, "GET", &wrong, "", "").await.0, 401);

    let mut s = TcpStream::connect(addr).await.unwrap();
    let request = format!("GET /j/{id}/api/events?code=ab2-3cd HTTP/1.1\r\nHost: x\r\n\r\n");
    s.write_all(request.as_bytes()).await.unwrap();
    let mut lines = BufReader::new(s).lines();
    // Laisse le flux s'ouvrir avant que l'hôte publie.
    tokio::time::sleep(Duration::from_millis(200)).await;
    send(
        &mut socket,
        &FromHost::State {
            data: r#"{"now":null,"paused":true,"queue":[]}"#.into(),
        },
    )
    .await;
    let data = loop {
        let line = tokio::time::timeout(Duration::from_secs(5), lines.next_line())
            .await
            .expect("aucun état reçu")
            .unwrap()
            .unwrap();
        if let Some(data) = line.strip_prefix("data: ") {
            break data.to_owned();
        }
    };
    assert_eq!(data, r#"{"now":null,"paused":true,"queue":[]}"#);
}

#[tokio::test]
async fn a_disconnected_host_can_resume_the_same_jam() {
    let addr = start(|_| {}).await;
    let (socket, id, secret, _) = host(addr, None).await;
    drop(socket);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let path = format!("/j/{id}/api/join");
    let (status, _, body) = http(addr, "POST", &path, CODE, "").await;
    assert_eq!((status, body.as_str()), (503, r#"{"error":"offline"}"#));
    // La page reste servie pendant le délai de reprise.
    assert_eq!(http(addr, "GET", &format!("/j/{id}/"), "", "").await.0, 200);

    // Mauvais secret : nouvelle Jam, pas de reprise.
    let wrong = Resume {
        id: id.clone(),
        secret: "faux".into(),
    };
    let (other, other_id, _, resumed) = host(addr, Some(wrong)).await;
    assert!(!resumed);
    assert_ne!(other_id, id);
    drop(other);

    let (socket, again, _, resumed) = host(
        addr,
        Some(Resume {
            id: id.clone(),
            secret,
        }),
    )
    .await;
    assert!(resumed);
    assert_eq!(again, id);
    let _host = echo(socket);
    assert_eq!(http(addr, "POST", &path, CODE, "").await.0, 200);
}

/// Changement de réseau (#253) : l'hôte revient alors que le relais croit encore
/// l'ancienne connexion ouverte. Même Jam, et les invités atteignent la nouvelle connexion.
#[tokio::test]
async fn a_host_resumes_while_its_old_connection_is_still_open() {
    let addr = start(|_| {}).await;
    let (old, id, secret, _) = host(addr, None).await;
    let resume = Resume {
        id: id.clone(),
        secret,
    };
    let (socket, again, _, resumed) = host(addr, Some(resume)).await;
    assert!(resumed);
    assert_eq!(again, id);

    let _host = echo(socket);
    let path = format!("/j/{id}/api/join");
    assert_eq!(http(addr, "POST", &path, CODE, "").await.0, 200);
    // L'ancienne connexion, fermée après coup, ne coupe pas la nouvelle.
    drop(old);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(http(addr, "POST", &path, CODE, "").await.0, 200);
}

#[tokio::test]
async fn one_address_cannot_fill_the_relay() {
    let addr = start(|c| c.max_jams_per_address = 2).await;
    let (_first, _, _, _) = host(addr, None).await;
    let (_second, _, _, _) = host(addr, None).await;
    let mut third = connect_host(addr, KEY).await.unwrap();
    send(
        &mut third,
        &FromHost::Hello {
            code: CODE.into(),
            resume: None,
        },
    )
    .await;
    assert_eq!(
        receive(&mut third).await,
        ToHost::Error {
            message: "trop de Jams ouvertes depuis cette adresse".into()
        }
    );
}

#[tokio::test]
async fn the_relay_refuses_jams_beyond_its_limits() {
    let addr = start(|c| {
        c.max_jams = 1;
        c.creations_per_hour = 2;
    })
    .await;
    let (_first, _, _, _) = host(addr, None).await;
    let mut second = connect_host(addr, KEY).await.unwrap();
    send(
        &mut second,
        &FromHost::Hello {
            code: CODE.into(),
            resume: None,
        },
    )
    .await;
    assert_eq!(
        receive(&mut second).await,
        ToHost::Error {
            message: "relais complet".into()
        }
    );
}

#[tokio::test]
async fn guests_are_rate_limited() {
    let addr = start(|c| c.requests_per_minute = 2).await;
    let (socket, id, _, _) = host(addr, None).await;
    let _host = echo(socket);
    let path = format!("/j/{id}/api/join");
    assert_eq!(http(addr, "POST", &path, CODE, "").await.0, 200);
    assert_eq!(http(addr, "POST", &path, CODE, "").await.0, 200);
    assert_eq!(http(addr, "POST", &path, CODE, "").await.0, 429);
}
