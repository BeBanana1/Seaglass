//! Tunnels d'appareils (#430) de bout en bout : un faux PC en WebSocket, son téléphone en
//! HTTP. Les corps sont opaques pour le relais : on vérifie qu'ils passent intacts.

use std::net::SocketAddr;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use seaglass_relay::protocol::{FromDevice, ToDevice};
use seaglass_relay::{Config, serve};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

const KEY: &str = "cle-de-test";
const ID: &str = "0123456789abcdef0123456789abcdef";
const SECRET: &str = "fedcba9876543210fedcba9876543210";

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn start() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(serve(listener, Config::new("https://jam.example/", KEY)));
    addr
}

async fn hello(addr: SocketAddr, id: &str, secret: &str) -> (Socket, ToDevice) {
    let mut request = format!("ws://{addr}/device").into_client_request().unwrap();
    request
        .headers_mut()
        .insert("authorization", format!("Bearer {KEY}").parse().unwrap());
    let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
    let hello = FromDevice::Hello {
        id: id.into(),
        secret: secret.into(),
    };
    socket
        .send(Message::Text(serde_json::to_string(&hello).unwrap().into()))
        .await
        .unwrap();
    let answer = receive(&mut socket).await;
    (socket, answer)
}

async fn receive(socket: &mut Socket) -> ToDevice {
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

/// Répond à chaque paquet par son inverse.
fn echo(mut socket: Socket) {
    tokio::spawn(async move {
        while let Some(Ok(message)) = socket.next().await {
            let Message::Text(text) = message else {
                continue;
            };
            let Ok(ToDevice::Request { rid, body, .. }) = serde_json::from_str(&text) else {
                continue;
            };
            let reply = FromDevice::Response {
                rid,
                status: 200,
                body: body.chars().rev().collect(),
            };
            let text = serde_json::to_string(&reply).unwrap();
            if socket.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });
}

async fn post(addr: SocketAddr, path: &str, body: &str) -> (u16, String) {
    let mut s = TcpStream::connect(addr).await.unwrap();
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    s.write_all(request.as_bytes()).await.unwrap();
    let mut out = Vec::new();
    s.read_to_end(&mut out).await.unwrap();
    let out = String::from_utf8(out).unwrap();
    let (head, body) = out.split_once("\r\n\r\n").unwrap();
    (head[9..12].parse().unwrap(), body.to_owned())
}

#[tokio::test]
async fn a_large_packet_goes_through_and_back() {
    let addr = start().await;
    let (socket, ready) = hello(addr, ID, SECRET).await;
    assert_eq!(
        ready,
        ToDevice::Ready {
            url: format!("https://jam.example/d/{ID}")
        }
    );
    echo(socket);
    // Bien plus que les 2 Kio des appels de la Jam.
    let packet: String = (0..3_000_000)
        .map(|i| char::from(b'a' + (i % 26) as u8))
        .collect();
    let (status, body) = post(addr, &format!("/d/{ID}"), &packet).await;
    assert_eq!(status, 200);
    assert_eq!(body, packet.chars().rev().collect::<String>());
}

#[tokio::test]
async fn an_absent_device_is_offline() {
    let addr = start().await;
    assert_eq!(post(addr, &format!("/d/{ID}"), "x").await.0, 404);
}

#[tokio::test]
async fn only_the_secret_takes_an_id_back() {
    let addr = start().await;
    let (_first, _) = hello(addr, ID, SECRET).await;
    let (_, refused) = hello(addr, ID, &"0".repeat(32)).await;
    assert!(matches!(refused, ToDevice::Error { .. }), "{refused:?}");
    // Le même PC qui revient prend la place de son ancienne connexion.
    let (again, ready) = hello(addr, ID, SECRET).await;
    assert!(matches!(ready, ToDevice::Ready { .. }));
    echo(again);
    assert_eq!(
        post(addr, &format!("/d/{ID}"), "abc").await,
        (200, "cba".into())
    );
    let (_, bad) = hello(addr, "court", SECRET).await;
    assert!(matches!(bad, ToDevice::Error { .. }));
}
