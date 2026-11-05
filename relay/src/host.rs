//! Connexion de l'app de l'hôte (`GET /host`, WebSocket).

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};

use crate::jams::{Jam, Reply};
use crate::limits::same;
use crate::protocol::{FromHost, ToHost};
use crate::{MAX_HOST_MESSAGE, Relay};

/// L'hôte doit dire `hello` dans ce délai.
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
/// Ping envoyé à l'hôte ; sans aucun message de sa part pendant `IDLE_TIMEOUT`, la
/// connexion est considérée comme perdue (la Jam attend sa reprise).
const PING_INTERVAL: Duration = Duration::from_secs(20);
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);

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
        .max_message_size(MAX_HOST_MESSAGE)
        .on_upgrade(move |socket| session(socket, relay, ip))
}

async fn session(socket: WebSocket, relay: Arc<Relay>, ip: std::net::IpAddr) {
    let (mut sink, mut stream) = socket.split();
    let hello = match tokio::time::timeout(HELLO_TIMEOUT, stream.next()).await {
        Ok(Some(Ok(Message::Text(text)))) => serde_json::from_str::<FromHost>(&text).ok(),
        _ => None,
    };
    let Some(FromHost::Hello { code, resume }) = hello else {
        return;
    };
    let resumed = resume
        .as_ref()
        .and_then(|r| relay.jams.resume(&r.id, &r.secret));
    let is_resume = resumed.is_some();
    let jam = match resumed {
        Some(jam) => jam,
        None => {
            let created = if Relay::allow(&relay.creations, ip) {
                relay
                    .jams
                    .create(code.clone(), relay.config.max_jams)
                    .ok_or("relais complet")
            } else {
                Err("trop de Jams créées depuis cette adresse")
            };
            match created {
                Ok(jam) => jam,
                Err(message) => {
                    let message = message.to_owned();
                    let _ = send(&mut sink, &ToHost::Error { message }).await;
                    return;
                }
            }
        }
    };
    let (mut outbox, generation) = jam.attach(code);
    tracing::info!(
        "Jam {} : hôte {} ({} ouvertes)",
        short(&jam.id),
        if is_resume { "revenu" } else { "connecté" },
        relay.jams.len()
    );
    let ready = ToHost::Ready {
        id: jam.id.clone(),
        secret: jam.secret().to_owned(),
        url: format!("{}/j/{}/", relay.config.public_url, jam.id),
        resumed: is_resume,
    };
    if send(&mut sink, &ready).await {
        run(&mut sink, &mut stream, &mut outbox, &jam).await;
    }
    jam.detach(generation);
    tracing::info!("Jam {} : hôte déconnecté", short(&jam.id));
}

/// Relaie dans les deux sens jusqu'à la fin de la connexion.
async fn run(
    sink: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    stream: &mut futures_util::stream::SplitStream<WebSocket>,
    outbox: &mut tokio::sync::mpsc::Receiver<ToHost>,
    jam: &Jam,
) {
    let mut ping = tokio::time::interval(PING_INTERVAL);
    let mut last_seen = tokio::time::Instant::now();
    loop {
        tokio::select! {
            incoming = stream.next() => {
                let Some(Ok(message)) = incoming else { return };
                last_seen = tokio::time::Instant::now();
                match message {
                    Message::Text(text) => handle(jam, &text),
                    Message::Close(_) => return,
                    _ => {}
                }
            }
            outgoing = outbox.recv() => {
                let Some(message) = outgoing else { return };
                let closing = matches!(message, ToHost::Error { .. });
                if !send(sink, &message).await || closing {
                    return;
                }
            }
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

fn handle(jam: &Jam, text: &Utf8Bytes) {
    match serde_json::from_str::<FromHost>(text) {
        Ok(FromHost::Response { rid, status, body }) => {
            jam.respond(rid, Reply { status, body });
        }
        Ok(FromHost::State { data }) => jam.push_state(data),
        // Un second `hello` ou un message illisible est ignoré.
        Ok(FromHost::Hello { .. }) | Err(_) => {}
    }
}

async fn send(
    sink: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    message: &ToHost,
) -> bool {
    let Ok(text) = serde_json::to_string(message) else {
        return false;
    };
    matches!(
        tokio::time::timeout(
            Duration::from_secs(10),
            sink.send(Message::Text(text.into()))
        )
        .await,
        Ok(Ok(()))
    )
}

/// Début de l'identifiant, pour les journaux (l'identifiant complet donne accès à la page).
fn short(id: &str) -> &str {
    id.get(..6).unwrap_or(id)
}
