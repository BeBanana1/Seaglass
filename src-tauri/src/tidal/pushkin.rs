//! Pushkin, TIDAL's push service (#35): how the official clients keep to one stream per
//! account. Read in `tidal-sdk-web` (`player/src/internal/services/pushkin.ts`) and checked
//! live with the official Android app (`spike/pushkin.py`, `docs/tidal.md`).
//!
//! `POST v1/rt/connect` gives a WebSocket URL. On an explicit Play, the client sends
//! `USER_ACTION`; the server then sends `PRIVILEGED_SESSION_NOTIFICATION` to the account's
//! other clients, which pause. The server enforces nothing itself.
//!
//! The connection follows playback, not the sign-in: it opens when playback starts and
//! closes a while after it stops (a notification only matters while playing, and the
//! socket needs a ping every 2 s). The URL is never logged, nor any error that could hold
//! it.

mod lifecycle;

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use reqwest::header::CONTENT_TYPE;
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_tungstenite::Connector;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};

use super::api::API_V1;
use super::models::PlaybackTaken;
use super::{Result, TidalClient, TidalError};
use lifecycle::Lifecycle;

/// The server drops a connection after 5 s without traffic (measured, no close frame).
const PING_INTERVAL: Duration = Duration::from_secs(2);
/// Nothing received (not even a pong) for this long: the connection is dead.
const SILENCE_TIMEOUT: Duration = Duration::from_secs(8);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const SEND_TIMEOUT: Duration = Duration::from_secs(5);
const MIN_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(60);
/// Longest device name kept (it ends up in a toast).
const MAX_DEVICE_CHARS: usize = 64;

/// What the player tells the connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Playback {
    /// Playback starts or resumes on an explicit Play: the other clients must stop.
    Claim,
    /// Playing without a new Play (gapless, next track): listen, claim nothing.
    Playing,
    /// Not playing anymore.
    Stopped,
}

/// Handle on the connection task; dropping it ends the task.
pub struct Pushkin {
    control: mpsc::UnboundedSender<Playback>,
}

impl Pushkin {
    /// Starts the connection task (idle until playback starts). Takeovers by another
    /// client come out of the receiver.
    pub fn start(tidal: Arc<TidalClient>) -> (Self, mpsc::Receiver<PlaybackTaken>) {
        let (control, control_rx) = mpsc::unbounded_channel();
        let (taken, taken_rx) = mpsc::channel(4);
        tauri::async_runtime::spawn(run(tidal, control_rx, taken));
        (Self { control }, taken_rx)
    }

    /// Never waits: playback does not depend on the connection.
    pub fn send(&self, playback: Playback) {
        let _ = self.control.send(playback);
    }
}

// --- Messages ----------------------------------------------------------------------

/// What the client sends on an explicit Play; `started_at` in Unix ms.
fn user_action(started_at: i64) -> String {
    serde_json::json!({ "type": "USER_ACTION", "payload": { "startedAt": started_at } }).to_string()
}

#[derive(Debug, PartialEq, Eq)]
enum Incoming {
    Taken(PlaybackTaken),
    /// Fetch a new URL and reconnect.
    Reconnect,
    /// Anything else, kept by its type for the logs.
    Other(String),
}

#[derive(Deserialize)]
struct RawIncoming {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    payload: Option<RawNotification>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawNotification {
    #[serde(default)]
    client_display_name: Option<String>,
}

fn parse(text: &str) -> Incoming {
    let Ok(raw) = serde_json::from_str::<RawIncoming>(text) else {
        return Incoming::Other("(not JSON)".to_owned());
    };
    match raw.kind.as_str() {
        "PRIVILEGED_SESSION_NOTIFICATION" => Incoming::Taken(PlaybackTaken {
            device: raw
                .payload
                .and_then(|p| p.client_display_name)
                .map(|name| {
                    name.trim()
                        .chars()
                        .take(MAX_DEVICE_CHARS)
                        .collect::<String>()
                })
                .filter(|name| !name.is_empty()),
        }),
        "RECONNECT" => Incoming::Reconnect,
        other => Incoming::Other(other.chars().take(64).collect()),
    }
}

// --- Connection --------------------------------------------------------------------

#[derive(Deserialize)]
struct RtConnect {
    url: String,
}

impl TidalClient {
    /// Pushkin's WebSocket URL for the current session, refreshing the token once on a
    /// 401. Never logged.
    async fn pushkin_url(&self) -> Result<String> {
        let mut rejected: Option<String> = None;
        loop {
            let (token, _) = self.credentials(rejected.as_deref()).await?;
            let resp = self
                .http
                .post(format!("{API_V1}rt/connect"))
                .bearer_auth(&token)
                .header(CONTENT_TYPE, "application/json")
                .send()
                .await?;
            let status = resp.status();
            if status.as_u16() == 401 && rejected.is_none() {
                rejected = Some(token);
                continue;
            }
            if !status.is_success() {
                return Err(TidalError::Api {
                    status: status.as_u16(),
                    message: "rt/connect".to_owned(),
                });
            }
            let connect: RtConnect = resp.json().await?;
            return Ok(connect.url);
        }
    }
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

enum Failure {
    /// No session: wait for the player.
    SignedOut,
    /// Network, server: retry with backoff.
    Retry(String),
}

async fn connect(tidal: &TidalClient) -> std::result::Result<Socket, Failure> {
    let url = match tidal.pushkin_url().await {
        Ok(url) => url,
        Err(TidalError::NotLoggedIn) => return Err(Failure::SignedOut),
        Err(e) => return Err(Failure::Retry(e.to_string())),
    };
    if !url.starts_with("wss://") {
        return Err(Failure::Retry("not a wss:// URL".to_owned()));
    }
    let request = url
        .as_str()
        .into_client_request()
        .map_err(|_| Failure::Retry("invalid URL".to_owned()))?;
    let tls = crate::tls::client().map_err(Failure::Retry)?;
    let connecting = tokio_tungstenite::connect_async_tls_with_config(
        request,
        None,
        false,
        Some(Connector::Rustls(tls)),
    );
    // Only a category of the error: its text may quote the URL.
    match tokio::time::timeout(CONNECT_TIMEOUT, connecting).await {
        Ok(Ok((socket, _))) => Ok(socket),
        Ok(Err(WsError::Http(response))) => Err(Failure::Retry(format!(
            "handshake refused ({})",
            response.status().as_u16()
        ))),
        Ok(Err(_)) => Err(Failure::Retry("handshake failed".to_owned())),
        Err(_) => Err(Failure::Retry("handshake timed out".to_owned())),
    }
}

/// How a connection ended.
enum End {
    /// No longer wanted (playback stopped long enough).
    Done,
    /// Lost or asked by the server: connect again.
    Lost,
    Reconnect,
    /// The player is gone.
    Shutdown,
}

async fn run(
    tidal: Arc<TidalClient>,
    mut control: mpsc::UnboundedReceiver<Playback>,
    taken: mpsc::Sender<PlaybackTaken>,
) {
    let mut life = Lifecycle::default();
    let mut backoff = MIN_BACKOFF;
    loop {
        while !life.wanted(Instant::now()) {
            match control.recv().await {
                Some(playback) => life.on(playback, Instant::now()),
                None => return,
            }
        }
        let socket = match connect(&tidal).await {
            Ok(socket) => socket,
            Err(Failure::SignedOut) => {
                life.give_up();
                continue;
            }
            Err(Failure::Retry(reason)) => {
                tracing::debug!("Pushkin: not connected ({reason}), retry in {backoff:?}");
                if !wait(backoff, &mut life, &mut control).await {
                    return;
                }
                backoff = (backoff * 2).min(MAX_BACKOFF);
                continue;
            }
        };
        tracing::debug!("Pushkin: connected");
        match connection(socket, &mut life, &mut control, &taken).await {
            End::Done => {
                tracing::debug!("Pushkin: closed, nothing playing");
                backoff = MIN_BACKOFF;
            }
            End::Reconnect => {
                tracing::debug!("Pushkin: reconnection asked");
                backoff = MIN_BACKOFF;
            }
            End::Lost => {
                tracing::debug!("Pushkin: connection lost, retry in {backoff:?}");
                if !wait(backoff, &mut life, &mut control).await {
                    return;
                }
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
            End::Shutdown => return,
        }
    }
}

/// Waits `delay` while following the player; `false` if it is gone.
async fn wait(
    delay: Duration,
    life: &mut Lifecycle,
    control: &mut mpsc::UnboundedReceiver<Playback>,
) -> bool {
    let deadline = Instant::now() + delay;
    loop {
        tokio::select! {
            () = tokio::time::sleep_until(deadline) => return true,
            playback = control.recv() => match playback {
                Some(playback) => life.on(playback, Instant::now()),
                None => return false,
            },
        }
    }
}

async fn connection(
    socket: Socket,
    life: &mut Lifecycle,
    control: &mut mpsc::UnboundedReceiver<Playback>,
    taken: &mpsc::Sender<PlaybackTaken>,
) -> End {
    let (mut sink, mut stream) = socket.split();
    let mut ping = tokio::time::interval(PING_INTERVAL);
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_seen = Instant::now();
    let end = loop {
        let now = Instant::now();
        if !life.wanted(now) {
            break End::Done;
        }
        if life.take_claim(now) {
            let message = Message::Text(user_action(now_ms()).into());
            if !sent(&mut sink, message).await {
                life.keep_claim();
                break End::Lost;
            }
            tracing::debug!("Pushkin: USER_ACTION sent");
        }
        let linger = life.lingers_until();
        tokio::select! {
            playback = control.recv() => match playback {
                Some(playback) => life.on(playback, Instant::now()),
                None => break End::Shutdown,
            },
            incoming = stream.next() => {
                let Some(Ok(message)) = incoming else { break End::Lost };
                last_seen = Instant::now();
                match message {
                    Message::Text(text) => match parse(&text) {
                        Incoming::Taken(event) => {
                            if life.ignores_notification(Instant::now()) {
                                tracing::debug!("Pushkin: notification right after our claim, ignored");
                            } else {
                                let _ = taken.try_send(event);
                            }
                        }
                        Incoming::Reconnect => break End::Reconnect,
                        Incoming::Other(kind) => tracing::debug!("Pushkin: unhandled message {kind}"),
                    },
                    Message::Close(_) => break End::Lost,
                    _ => {}
                }
            }
            _ = ping.tick() => {
                if last_seen.elapsed() >= SILENCE_TIMEOUT
                    || !sent(&mut sink, Message::Ping(Default::default())).await
                {
                    break End::Lost;
                }
            }
            () = sleep_until(linger), if linger.is_some() => {}
        }
    };
    if matches!(end, End::Done | End::Reconnect | End::Shutdown) {
        let _ = sent(&mut sink, Message::Close(None)).await;
    }
    end
}

async fn sleep_until(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

async fn sent(
    sink: &mut futures_util::stream::SplitSink<Socket, Message>,
    message: Message,
) -> bool {
    matches!(
        tokio::time::timeout(SEND_TIMEOUT, sink.send(message)).await,
        Ok(Ok(()))
    )
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_action_is_the_sdk_message() {
        let sent: serde_json::Value =
            serde_json::from_str(&user_action(1_795_622_116_388)).unwrap();
        assert_eq!(
            sent,
            serde_json::json!({"type": "USER_ACTION", "payload": {"startedAt": 1_795_622_116_388_i64}})
        );
    }

    #[test]
    fn a_notification_names_the_other_device() {
        // As received from the official Android app on 2026-11-25.
        let text = r#"{"payload":{"clientDisplayName":"Android","sessionId":"3bd295a2-bd99-4013-899b-bf2da212c95f"},"type":"PRIVILEGED_SESSION_NOTIFICATION"}"#;
        assert_eq!(
            parse(text),
            Incoming::Taken(PlaybackTaken {
                device: Some("Android".to_owned())
            })
        );
    }

    #[test]
    fn a_notification_without_a_usable_name_still_counts() {
        for payload in [
            r#"{"type":"PRIVILEGED_SESSION_NOTIFICATION"}"#,
            r#"{"type":"PRIVILEGED_SESSION_NOTIFICATION","payload":{"clientDisplayName":null}}"#,
            r#"{"type":"PRIVILEGED_SESSION_NOTIFICATION","payload":{"clientDisplayName":"  "}}"#,
        ] {
            assert_eq!(
                parse(payload),
                Incoming::Taken(PlaybackTaken { device: None }),
                "{payload}"
            );
        }
        let long = format!(
            r#"{{"type":"PRIVILEGED_SESSION_NOTIFICATION","payload":{{"clientDisplayName":"{}"}}}}"#,
            "x".repeat(500)
        );
        let Incoming::Taken(PlaybackTaken { device: Some(name) }) = parse(&long) else {
            panic!("not a takeover");
        };
        assert_eq!(name.chars().count(), MAX_DEVICE_CHARS);
    }

    #[test]
    fn other_messages_are_told_apart() {
        assert_eq!(parse(r#"{"type":"RECONNECT"}"#), Incoming::Reconnect);
        assert_eq!(
            parse(r#"{"type":"SOMETHING_NEW","payload":{}}"#),
            Incoming::Other("SOMETHING_NEW".to_owned())
        );
        assert!(matches!(parse("not json"), Incoming::Other(_)));
    }

    /// `cargo test live_pushkin_stays_open -- --ignored --nocapture`: connects under the
    /// stored session and keeps the socket 12 s with the pings, sending no `USER_ACTION`
    /// (nothing else on the account is paused).
    #[tokio::test]
    #[ignore = "needs the network and a TIDAL session"]
    async fn live_pushkin_stays_open() {
        let tidal = super::super::live_tests::client().await;
        let Ok(socket) = connect(&tidal).await else {
            panic!("not connected");
        };
        let (mut sink, mut stream) = socket.split();
        let deadline = Instant::now() + Duration::from_secs(12);
        let mut ping = tokio::time::interval(PING_INTERVAL);
        let mut pongs = 0;
        loop {
            tokio::select! {
                () = tokio::time::sleep_until(deadline) => break,
                _ = ping.tick() => assert!(sent(&mut sink, Message::Ping(Default::default())).await),
                incoming = stream.next() => match incoming {
                    Some(Ok(Message::Pong(_))) => pongs += 1,
                    Some(Ok(Message::Text(text))) => println!("received {:?}", parse(&text)),
                    Some(Ok(_)) => {}
                    _ => panic!("connection dropped after {pongs} pong(s)"),
                },
            }
        }
        println!("open for 12 s, {pongs} pong(s)");
        assert!(pongs >= 5);
    }
}
