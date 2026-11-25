//! Device tunnels (#430) end to end: a fake PC over WebSocket, its phone over HTTP. Bodies
//! are opaque to the relay: the tests check that they go through intact.

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
            .expect("nothing received from the relay")
            .unwrap()
            .unwrap();
        if let Message::Text(text) = message {
            return serde_json::from_str(&text).unwrap();
        }
    }
}

/// Answers each packet with its reverse.
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
    // Far more than the 2 KiB of Jam calls.
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
    // The same PC coming back takes the place of its old connection.
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

/// Sends only the head of a POST declaring `length` bytes, then reads the answer: the relay
/// must answer without waiting for a body it will not read.
async fn head_only(addr: SocketAddr, path: &str, length: usize) -> u16 {
    let mut s = TcpStream::connect(addr).await.unwrap();
    let head = format!(
        "POST {path} HTTP/1.1
Host: x
Content-Length: {length}

"
    );
    s.write_all(head.as_bytes()).await.unwrap();
    let mut out = [0u8; 12];
    tokio::time::timeout(Duration::from_secs(5), s.read_exact(&mut out))
        .await
        .expect("the relay waited for the body")
        .unwrap();
    std::str::from_utf8(&out[9..12]).unwrap().parse().unwrap()
}

#[tokio::test]
async fn bodies_are_not_read_for_an_absent_device() {
    let addr = start().await;
    assert_eq!(
        head_only(addr, &format!("/d/{ID}"), 8 * 1024 * 1024).await,
        404
    );
}

#[tokio::test]
async fn packets_beyond_the_limit_are_refused_unread() {
    let addr = start().await;
    let (socket, _) = hello(addr, ID, SECRET).await;
    echo(socket);
    assert_eq!(
        head_only(addr, &format!("/d/{ID}"), 8 * 1024 * 1024 + 1).await,
        413
    );
}

/// Waits for the relay to close `socket`.
async fn closed(socket: &mut Socket) -> bool {
    loop {
        match tokio::time::timeout(Duration::from_secs(5), socket.next()).await {
            Err(_) => return false,
            Ok(None | Some(Err(_)) | Some(Ok(Message::Close(_)))) => return true,
            Ok(Some(Ok(_))) => {}
        }
    }
}

#[tokio::test]
async fn an_oversized_answer_closes_the_tunnel() {
    let addr = start().await;
    let (mut socket, _) = hello(addr, ID, SECRET).await;
    let phone = tokio::spawn(async move { post(addr, &format!("/d/{ID}"), "x").await });
    let ToDevice::Request { rid, .. } = receive(&mut socket).await else {
        panic!("no request");
    };
    let reply = FromDevice::Response {
        rid,
        status: 200,
        body: "a".repeat(4 * 1024 * 1024),
    };
    let text = serde_json::to_string(&reply).unwrap();
    let _ = socket.send(Message::Text(text.into())).await;
    assert!(closed(&mut socket).await, "the relay kept the connection");
    phone.abort();
}

#[tokio::test]
async fn a_message_sent_while_idle_does_not_answer_the_next_request() {
    let addr = start().await;
    let (mut socket, _) = hello(addr, ID, SECRET).await;
    // Nobody asked: not read now, then read and dropped (unknown `rid`) once a request
    // waits, before the real answer. (Request ids only grow: a late answer of the PC to a
    // request that timed out always carries an older one.)
    for rid in [998, 999] {
        let stray = FromDevice::Response {
            rid,
            status: 200,
            body: "stray".into(),
        };
        let text = serde_json::to_string(&stray).unwrap();
        socket.send(Message::Text(text.into())).await.unwrap();
    }
    echo(socket);
    assert_eq!(
        post(addr, &format!("/d/{ID}"), "abc").await,
        (200, "cba".into())
    );
}
