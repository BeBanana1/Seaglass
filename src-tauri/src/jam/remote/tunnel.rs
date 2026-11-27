//! The PC's tunnel on the relay (#430): the linked phone reaches it away from the local
//! network for the playback handoff and the listening history.
//!
//! The PC connects outbound to `/device` with an id and a secret it generated (kept with
//! the remote control settings); each call from the phone arrives encrypted
//! (`handoff::sealed`), is decrypted here then served by the same API as on the local
//! network (code, limits, rights), and the response goes back encrypted. Only the phone's
//! routes get through: never the page nor the remote control's commands.

use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_tungstenite::Connector;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};

use super::RemoteSite;
use super::api::{Owner, Remote};
use crate::handoff::sealed::{self, Replay, TunnelKey};
use crate::jam::protocol::{FromDevice, ToDevice};
use crate::jam::relay::RelayConfig;
use crate::tls;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const READY_TIMEOUT: Duration = Duration::from_secs(10);
const PING_INTERVAL: Duration = Duration::from_secs(20);
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_BACKOFF: Duration = Duration::from_secs(30);
/// Leaves the PC time to answer before the relay's own timeout (30 s).
const ANSWER_TIMEOUT: Duration = Duration::from_secs(25);
/// A lasting error (key refused, id taken): retry, but rarely.
const FAILED_BACKOFF: Duration = Duration::from_secs(10 * 60);

/// The PC's identity on the relay and the key shared with the phone, kept in the remote
/// control settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunnelSettings {
    pub id: String,
    pub secret: String,
    /// ChaCha20-Poly1305 key in hexadecimal; changed with the code ("New code").
    pub key: String,
}

impl TunnelSettings {
    pub fn new() -> Self {
        Self {
            id: sealed::new_token(),
            secret: sealed::new_token(),
            key: sealed::new_key(),
        }
    }

    /// Settings read back: `None` if they are damaged (new ones are generated).
    pub fn checked(self) -> Option<Self> {
        let token = |s: &str| s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit());
        (token(&self.id) && token(&self.secret) && TunnelKey::parse(&self.key).is_some())
            .then_some(self)
    }
}

/// What the phone learns on the local network to go through the relay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayRoute {
    /// `https://<relais>/d/<id>`.
    pub url: String,
    pub key: String,
}

/// Routes served by the tunnel.
fn allowed(path: &str) -> bool {
    path == "/api/join" || path.starts_with("/api/handoff/") || path.starts_with("/api/plays/")
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// The tunnel decrypts, checks and serves a packet; returns (relay status, body).
async fn answer<R: Remote>(
    owner: &Owner,
    remote: &R,
    key: &TunnelKey,
    tunnel: &str,
    replay: &Mutex<Replay>,
    ip: IpAddr,
    packet: &str,
) -> (u16, String) {
    let Some((request, nonce)) = sealed::open_request(key, tunnel, packet) else {
        // Wrong key (phone linked before a "New code") or damaged packet.
        return (403, r#"{"error":"key"}"#.to_owned());
    };
    let fresh = replay
        .lock()
        .map(|mut r| r.accept(nonce, request.at, now_ms()))
        .unwrap_or(false);
    let response = if !fresh {
        sealed::Response {
            status: 409,
            body: r#"{"error":"replay"}"#.to_owned(),
        }
    } else if !allowed(&request.path) {
        sealed::Response {
            status: 404,
            body: r#"{"error":"not_found"}"#.to_owned(),
        }
    } else {
        let served = tokio::time::timeout(
            ANSWER_TIMEOUT,
            owner.api(
                &request.path,
                Some(&request.code),
                request.body.as_bytes(),
                ip,
                remote,
            ),
        )
        .await;
        match served {
            Ok(r) => sealed::Response {
                status: r.status,
                body: String::from_utf8_lossy(&r.body).into_owned(),
            },
            Err(_) => sealed::Response {
                status: 502,
                body: r#"{"error":"backend"}"#.to_owned(),
            },
        }
    };
    (200, sealed::seal_response(key, tunnel, &nonce, &response))
}

enum Failure {
    /// Retry soon (network).
    Retry,
    /// Refused by the relay (build key, id taken): retry rarely.
    Refused(String),
}

/// Keeps the tunnel open until the task is cancelled. The online address is given to
/// `Owner` once the relay is ready (`None` during outages).
pub(super) async fn run<R: Remote>(
    config: RelayConfig,
    settings: TunnelSettings,
    site: Arc<RemoteSite<R>>,
) {
    let Some(key) = TunnelKey::parse(&settings.key) else {
        return;
    };
    let replay = Arc::new(Mutex::new(Replay::default()));
    let mut backoff = Duration::from_secs(1);
    loop {
        let result = session(&config, &settings, &key, &site, &replay).await;
        site.owner.set_relay(None);
        let wait = match result {
            Ok(()) => {
                backoff = Duration::from_secs(1);
                backoff
            }
            Err(Failure::Retry) => backoff,
            Err(Failure::Refused(reason)) => {
                tracing::warn!("Relay tunnel refused: {reason}");
                FAILED_BACKOFF
            }
        };
        tokio::time::sleep(wait).await;
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn session<R: Remote>(
    config: &RelayConfig,
    settings: &TunnelSettings,
    key: &TunnelKey,
    site: &Arc<RemoteSite<R>>,
    replay: &Arc<Mutex<Replay>>,
) -> Result<(), Failure> {
    let url = config.device_url();
    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|e| Failure::Refused(e.to_string()))?;
    let bearer = HeaderValue::from_str(&format!("Bearer {}", config.key))
        .map_err(|e| Failure::Refused(e.to_string()))?;
    request.headers_mut().insert("authorization", bearer);
    let connector = if url.starts_with("wss://") {
        Some(Connector::Rustls(tls::client().map_err(Failure::Refused)?))
    } else {
        None
    };
    let connecting =
        tokio_tungstenite::connect_async_tls_with_config(request, None, false, connector);
    let socket: Socket = match tokio::time::timeout(CONNECT_TIMEOUT, connecting).await {
        Ok(Ok((socket, _))) => socket,
        Ok(Err(WsError::Http(response))) if response.status() == 401 => {
            return Err(Failure::Refused("relay key refused".into()));
        }
        // Relay not up to date yet (no `/device`): nothing to do before it is redeployed.
        Ok(Err(WsError::Http(response))) if response.status() == 404 => {
            return Err(Failure::Refused("relay without device tunnels".into()));
        }
        Ok(Err(e)) => {
            tracing::debug!("Tunnel : {e}");
            return Err(Failure::Retry);
        }
        Err(_) => return Err(Failure::Retry),
    };
    let (mut sink, mut stream) = socket.split();
    let hello = FromDevice::Hello {
        id: settings.id.clone(),
        secret: settings.secret.clone(),
    };
    send(&mut sink, &hello).await?;
    let ready = loop {
        match tokio::time::timeout(READY_TIMEOUT, stream.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                break serde_json::from_str::<ToDevice>(&text).map_err(|_| Failure::Retry)?;
            }
            Ok(Some(Ok(_))) => {}
            _ => return Err(Failure::Retry),
        }
    };
    match ready {
        ToDevice::Ready { url } => {
            tracing::info!("Relay tunnel open");
            site.owner.set_relay(Some(RelayRoute {
                url,
                key: settings.key.clone(),
            }));
        }
        ToDevice::Error { message } => return Err(Failure::Refused(message)),
        ToDevice::Request { .. } => return Err(Failure::Retry),
    }

    let (replies, mut outbox) = mpsc::channel::<FromDevice>(16);
    let mut ping = tokio::time::interval(PING_INTERVAL);
    let mut last_seen = Instant::now();
    loop {
        tokio::select! {
            incoming = stream.next() => {
                let Some(Ok(message)) = incoming else { return Ok(()) };
                last_seen = Instant::now();
                match message {
                    Message::Text(text) => match serde_json::from_str::<ToDevice>(&text) {
                        Ok(ToDevice::Request { rid, client, body }) => {
                            // Address seen by the relay: the PC's limits count it.
                            let ip = client.parse().unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
                            let (site, replay) = (Arc::clone(site), Arc::clone(replay));
                            let (key, tunnel, replies) =
                                (key.clone(), settings.id.clone(), replies.clone());
                            tokio::spawn(async move {
                                let (status, body) =
                                    answer(&site.owner, &site.remote, &key, &tunnel, &replay, ip, &body).await;
                                let _ = replies.send(FromDevice::Response { rid, status, body }).await;
                            });
                        }
                        Ok(ToDevice::Error { message }) => {
                            tracing::info!("Tunnel : {message}");
                            return Ok(());
                        }
                        _ => {}
                    },
                    Message::Close(_) => return Ok(()),
                    _ => {}
                }
            }
            reply = outbox.recv() => {
                if let Some(reply) = reply
                    && send(&mut sink, &reply).await.is_err()
                {
                    return Ok(());
                }
            }
            _ = ping.tick() => {
                if last_seen.elapsed() >= IDLE_TIMEOUT {
                    return Ok(());
                }
                let ping = sink.send(Message::Ping(Default::default()));
                if !matches!(tokio::time::timeout(Duration::from_secs(10), ping).await, Ok(Ok(()))) {
                    return Ok(());
                }
            }
        }
    }
}

async fn send(
    sink: &mut futures_util::stream::SplitSink<Socket, Message>,
    message: &FromDevice,
) -> Result<(), Failure> {
    let text = serde_json::to_string(message).map_err(|_| Failure::Retry)?;
    match tokio::time::timeout(
        Duration::from_secs(30),
        sink.send(Message::Text(text.into())),
    )
    .await
    {
        Ok(Ok(())) => Ok(()),
        _ => Err(Failure::Retry),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_are_checked() {
        let fresh = TunnelSettings::new();
        assert_eq!(fresh.clone().checked(), Some(fresh.clone()));
        assert!(
            TunnelSettings {
                key: "abc".into(),
                ..fresh.clone()
            }
            .checked()
            .is_none()
        );
        assert!(
            TunnelSettings {
                id: "court".into(),
                ..fresh
            }
            .checked()
            .is_none()
        );
    }

    #[test]
    fn only_the_phone_routes_pass() {
        assert!(allowed("/api/join"));
        assert!(allowed("/api/handoff/take"));
        assert!(allowed("/api/plays/sync"));
        for path in ["/api/next", "/api/state", "/api/search", "/api/relay", "/"] {
            assert!(!allowed(path), "{path}");
        }
    }
}
