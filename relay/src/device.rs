//! Connection of a PC (`GET /device`, WebSocket) and calls from its paired phone
//! (`POST /d/<id>`), #430. Bodies are end-to-end encrypted: the relay passes them as is.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};

use crate::devices::{AttachError, Device, is_token};
use crate::jams::{ForwardError, Reply};
use crate::limits::same;
use crate::protocol::{FromDevice, ToDevice};
use crate::{MAX_DEVICE_BODY, MAX_DEVICE_MESSAGE, Relay, content_length, into_text, read_body};

const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
const PING_INTERVAL: Duration = Duration::from_secs(20);
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);

type Sink = SplitSink<WebSocket, Message>;

pub async fn connect(
    State(relay): State<Arc<Relay>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    let key = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if !same(key, &relay.config.host_key) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let ip = relay.client_ip(peer, &headers);
    upgrade
        .max_message_size(MAX_DEVICE_MESSAGE)
        .on_upgrade(move |socket| session(socket, relay, ip))
}

async fn session(socket: WebSocket, relay: Arc<Relay>, ip: IpAddr) {
    let (mut sink, mut stream) = socket.split();
    let hello = match tokio::time::timeout(HELLO_TIMEOUT, stream.next()).await {
        Ok(Some(Ok(Message::Text(text)))) => serde_json::from_str::<FromDevice>(&text).ok(),
        _ => None,
    };
    let Some(FromDevice::Hello { id, secret }) = hello else {
        return;
    };
    let refuse = |message: &str| ToDevice::Error {
        message: message.to_owned(),
    };
    if !is_token(&id) || !is_token(&secret) {
        let _ = send(&mut sink, &refuse("invalid identifier")).await;
        return;
    }
    let attached = relay.devices.attach(
        &id,
        &secret,
        ip,
        relay.config.max_devices,
        relay.config.max_devices_per_address,
    );
    let (device, mut outbox, generation) = match attached {
        Ok(attached) => attached,
        Err(e) => {
            let message = match e {
                AttachError::Taken => "identifier already in use",
                AttachError::Full => "relay full",
                AttachError::TooManyForAddress => "too many devices from this address",
            };
            let _ = send(&mut sink, &refuse(message)).await;
            return;
        }
    };
    tracing::info!(
        "Device {} connected ({} in all)",
        short(&id),
        relay.devices.len()
    );
    let ready = ToDevice::Ready {
        url: format!("{}/d/{id}", relay.config.public_url),
    };
    if send(&mut sink, &ready).await {
        run(&mut sink, &mut stream, &mut outbox, &device).await;
    }
    relay.devices.detach(&id, generation);
    tracing::info!("Device {} disconnected", short(&id));
}

async fn run(
    sink: &mut Sink,
    stream: &mut SplitStream<WebSocket>,
    outbox: &mut tokio::sync::mpsc::Receiver<ToDevice>,
    device: &Device,
) {
    let mut ping = tokio::time::interval(PING_INTERVAL);
    let mut last_seen = tokio::time::Instant::now();
    loop {
        tokio::select! {
            incoming = stream.next() => {
                let Some(Ok(message)) = incoming else { return };
                last_seen = tokio::time::Instant::now();
                match message {
                    Message::Text(text) => {
                        if let Ok(FromDevice::Response { rid, status, body }) =
                            serde_json::from_str::<FromDevice>(&text)
                        {
                            device.respond(rid, Reply { status, body });
                        }
                    }
                    Message::Close(_) => return,
                    _ => {}
                }
            }
            outgoing = outbox.recv() => {
                let Some(message) = outgoing else { return };
                if !send(sink, &message).await {
                    return;
                }
            }
            () = device.replaced.notified() => return,
            _ = ping.tick() => {
                if last_seen.elapsed() >= IDLE_TIMEOUT
                    || sink.send(Message::Ping(Default::default())).await.is_err()
                {
                    return;
                }
            }
        }
    }
}

async fn send(sink: &mut Sink, message: &ToDevice) -> bool {
    let Ok(text) = serde_json::to_string(message) else {
        return false;
    };
    matches!(
        tokio::time::timeout(
            Duration::from_secs(30),
            sink.send(Message::Text(text.into()))
        )
        .await,
        Ok(Ok(()))
    )
}

fn reply(status: StatusCode, body: String) -> Response {
    let mut response = (status, body).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn error(status: StatusCode, kind: &str) -> Response {
    let mut response = (status, serde_json::json!({ "error": kind }).to_string()).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response
}

/// KiB of `Relay::device_uploads` a packet reserves: its declared length, the maximum
/// without one.
fn reservation(headers: &HeaderMap) -> u32 {
    let bytes = content_length(headers).map_or(MAX_DEVICE_BODY, |n| n.min(MAX_DEVICE_BODY));
    u32::try_from(bytes.div_ceil(1024).max(1)).unwrap_or(u32::MAX)
}

/// Call from the phone: encrypted packet forwarded to the PC, its answer sent back as is.
/// 404 `offline`: the PC is not connected. The body is only read once the rate limit, the
/// device and the memory budget have let the call through.
pub async fn call(
    State(relay): State<Arc<Relay>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let ip = relay.client_ip(peer, &headers);
    if !Relay::allow(&relay.device_calls, ip) {
        return error(StatusCode::TOO_MANY_REQUESTS, "slow");
    }
    let Some(device) = relay.devices.get(&id) else {
        return error(StatusCode::NOT_FOUND, "offline");
    };
    // Held until the answer: the packet stays in memory while the PC handles it.
    let Ok(_reserved) = relay.device_uploads.try_acquire_many(reservation(&headers)) else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "busy");
    };
    let Some(body) = read_body(body, &headers, MAX_DEVICE_BODY).await else {
        return error(StatusCode::PAYLOAD_TOO_LARGE, "too_large");
    };
    let Some(body) = into_text(body) else {
        return error(StatusCode::BAD_REQUEST, "body");
    };
    match device.forward(ip, body).await {
        Ok(answer) => reply(
            StatusCode::from_u16(answer.status).unwrap_or(StatusCode::BAD_GATEWAY),
            answer.body,
        ),
        Err(ForwardError::Offline) => error(StatusCode::NOT_FOUND, "offline"),
        Err(ForwardError::Busy) => error(StatusCode::SERVICE_UNAVAILABLE, "busy"),
    }
}

fn short(id: &str) -> &str {
    id.get(..6).unwrap_or(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packets_reserve_their_declared_size() {
        let headers = |length: &str| {
            let mut h = HeaderMap::new();
            h.insert("content-length", length.parse().unwrap());
            h
        };
        assert_eq!(reservation(&headers("0")), 1);
        assert_eq!(reservation(&headers("1025")), 2);
        assert_eq!(reservation(&HeaderMap::new()), 8 * 1024);
        assert_eq!(reservation(&headers("999999999")), 8 * 1024);
    }
}
