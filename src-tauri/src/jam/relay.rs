//! Online Jam (#25, step 3b): the app connects to the relay (`relay/`) over an outgoing
//! WebSocket, with no port open on the host's side. The guests' calls arrive as `request`,
//! go through the same core as the local Jam (`api.rs`) and leave as `response`; the state
//! is pushed to the relay when it changes.
//!
//! Cut (mobile network, sleep): reconnection at increasing intervals (1 s to 30 s) with the
//! relay's resume, which keeps the same address and the same QR code for 30 min (since
//! 0.4.85). Beyond that, the relay opens a new Jam: the address changes and the UI is told.

use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_tungstenite::Connector;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};

use super::api::{BACKEND_TIMEOUT, Backend, Core};
use super::protocol::{FromHost, Resume, ToHost};
use super::server::PUSH_INTERVAL;
use crate::i18n::{tr, trf};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const READY_TIMEOUT: Duration = Duration::from_secs(10);
const PING_INTERVAL: Duration = Duration::from_secs(20);
/// With no message from the relay for this long, the connection is lost.
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct RelayConfig {
    /// `wss://…/host` (or `ws://` for a local test relay).
    pub url: String,
    pub key: String,
}

impl RelayConfig {
    /// The Settings relay, otherwise the built-in one (#452, see `relay_settings`). `None`
    /// without a relay: only the local Jam is offered.
    pub fn current() -> Option<Self> {
        super::relay_settings::config()
    }

    /// Device tunnel (#430): same relay, `/device` instead of `/host`.
    pub fn device_url(&self) -> String {
        match self.url.strip_suffix("/host") {
            Some(base) => format!("{base}/device"),
            None => format!("{}/device", self.url.trim_end_matches('/')),
        }
    }
}

/// What the connection announces to `JamState` (address, state for the UI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelayEvent {
    Connecting,
    /// Jam open (or resumed with the same address if `resumed`).
    Ready {
        url: String,
        resumed: bool,
    },
    /// Connection lost or refused for now: trying again.
    Lost {
        reason: Option<String>,
    },
    /// Final refusal (key refused): the online Jam stops.
    Failed {
        reason: String,
    },
}

pub type Notify = Arc<dyn Fn(RelayEvent) + Send + Sync>;

enum Failure {
    Retry(Option<String>),
    Fatal(String),
}

/// Keeps the Jam open on the relay until the task is cancelled.
pub async fn run<B: Backend>(
    config: RelayConfig,
    code: String,
    core: Arc<Core>,
    backend: Arc<B>,
    notify: Notify,
) {
    let mut resume: Option<Resume> = None;
    let mut backoff = Duration::from_secs(1);
    loop {
        notify(RelayEvent::Connecting);
        match session(&config, &code, &core, &backend, &mut resume, &notify).await {
            // Connection established then lost: go again quickly.
            Ok(()) => {
                backoff = Duration::from_secs(1);
                notify(RelayEvent::Lost { reason: None });
            }
            Err(Failure::Retry(reason)) => notify(RelayEvent::Lost { reason }),
            Err(Failure::Fatal(reason)) => {
                tracing::warn!("Online Jam stopped: {reason}");
                notify(RelayEvent::Failed { reason });
                return;
            }
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

fn unreachable_relay() -> Failure {
    Failure::Retry(Some(
        tr("relais injoignable", "relay unreachable").to_owned(),
    ))
}

/// One connection: `Ok` if it was established (Jam ready) then lost.
async fn session<B: Backend>(
    config: &RelayConfig,
    code: &str,
    core: &Arc<Core>,
    backend: &Arc<B>,
    resume: &mut Option<Resume>,
    notify: &Notify,
) -> Result<(), Failure> {
    let mut request = config
        .url
        .as_str()
        .into_client_request()
        .map_err(|e| Failure::Fatal(e.to_string()))?;
    let bearer = HeaderValue::from_str(&format!("Bearer {}", config.key))
        .map_err(|e| Failure::Fatal(e.to_string()))?;
    request.headers_mut().insert("authorization", bearer);
    let connector = if config.url.starts_with("wss://") {
        Some(Connector::Rustls(
            crate::tls::client().map_err(Failure::Fatal)?,
        ))
    } else {
        None
    };
    let connecting =
        tokio_tungstenite::connect_async_tls_with_config(request, None, false, connector);
    let socket = match tokio::time::timeout(CONNECT_TIMEOUT, connecting).await {
        Ok(Ok((socket, _))) => socket,
        Ok(Err(WsError::Http(response))) if response.status() == 401 => {
            return Err(Failure::Fatal(
                tr("clé du relais refusée", "relay key refused").to_owned(),
            ));
        }
        Ok(Err(e)) => {
            tracing::info!("relay: {e}");
            return Err(unreachable_relay());
        }
        Err(_) => return Err(unreachable_relay()),
    };
    let (mut sink, mut stream) = socket.split();

    let hello = FromHost::Hello {
        code: code.to_owned(),
        resume: resume.clone(),
    };
    send(&mut sink, &hello).await?;
    let ready = loop {
        let message = match tokio::time::timeout(READY_TIMEOUT, stream.next()).await {
            Ok(Some(Ok(message))) => message,
            _ => return Err(unreachable_relay()),
        };
        if let Message::Text(text) = message {
            break serde_json::from_str::<ToHost>(&text).map_err(|_| unreachable_relay())?;
        }
    };
    match ready {
        ToHost::Ready {
            id,
            secret,
            url,
            resumed,
        } => {
            *resume = Some(Resume { id, secret });
            tracing::info!("Online Jam {}", if resumed { "resumed" } else { "opened" });
            notify(RelayEvent::Ready { url, resumed });
        }
        // Relay full, too many Jams from this address: try again later.
        ToHost::Error { message } => {
            return Err(Failure::Retry(Some(trf!(
                "refusé par le relais : {}",
                "refused by the relay: {}",
                message
            ))));
        }
        ToHost::Request { .. } => return Err(unreachable_relay()),
    }

    let (replies, mut outbox) = mpsc::channel::<FromHost>(64);
    let mut push = tokio::time::interval(PUSH_INTERVAL);
    let mut ping = tokio::time::interval(PING_INTERVAL);
    let mut last_seen = Instant::now();
    let mut last_state = String::new();
    loop {
        tokio::select! {
            incoming = stream.next() => {
                let Some(Ok(message)) = incoming else { return Ok(()) };
                last_seen = Instant::now();
                match message {
                    Message::Text(text) => match serde_json::from_str::<ToHost>(&text) {
                        Ok(ToHost::Request { rid, client, path, code, body }) => {
                            let ip = client.parse().unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
                            let core = Arc::clone(core);
                            let backend = Arc::clone(backend);
                            let replies = replies.clone();
                            tokio::spawn(async move {
                                let answer = tokio::time::timeout(
                                    BACKEND_TIMEOUT,
                                    core.api(&path, code.as_deref(), body.as_bytes(), ip, &*backend),
                                )
                                .await;
                                let (status, body) = match answer {
                                    Ok(r) => (r.status, String::from_utf8_lossy(&r.body).into_owned()),
                                    Err(_) => (502, r#"{"error":"backend"}"#.to_owned()),
                                };
                                let _ = replies.send(FromHost::Response { rid, status, body }).await;
                            });
                        }
                        // Jam closed by the relay (12 h): the resume will fail, new Jam.
                        Ok(ToHost::Error { message }) => {
                            tracing::info!("relay: {message}");
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
            _ = push.tick() => {
                if let Some(state) = core.state_json(&**backend).await
                    && state != last_state
                {
                    if send(&mut sink, &FromHost::State { data: state.clone() }).await.is_err() {
                        return Ok(());
                    }
                    last_state = state;
                }
            }
            () = core.refresh.notified() => push.reset_immediately(),
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

type Sink = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

async fn send(sink: &mut Sink, message: &FromHost) -> Result<(), Failure> {
    let text = serde_json::to_string(message).map_err(|e| Failure::Fatal(e.to_string()))?;
    match tokio::time::timeout(
        Duration::from_secs(10),
        sink.send(Message::Text(text.into())),
    )
    .await
    {
        Ok(Ok(())) => Ok(()),
        _ => Err(Failure::Retry(None)),
    }
}

#[cfg(test)]
// tungstenite's handshake error response is big: unimportant here.
#[allow(clippy::result_large_err)]
mod tests {
    use super::*;
    use crate::jam::testing::Fake;
    use std::sync::Mutex;
    use tokio::net::{TcpListener, TcpStream};
    use tokio_tungstenite::WebSocketStream;
    use tokio_tungstenite::tungstenite::handshake::server::{
        ErrorResponse, Request, Response as HandshakeResponse,
    };

    const CODE: &str = "AB23CD";
    const KEY: &str = "cle-de-test";

    type Relay = WebSocketStream<TcpStream>;

    /// Accepts a host while checking its key, like the relay.
    async fn accept(listener: &TcpListener) -> Result<Relay, ()> {
        let (stream, _) = listener.accept().await.unwrap();
        let check = |request: &Request, response: HandshakeResponse| {
            let auth = request.headers().get("authorization");
            if auth.and_then(|v| v.to_str().ok()) == Some(&format!("Bearer {KEY}")) {
                Ok(response)
            } else {
                let mut refused = ErrorResponse::new(None);
                *refused.status_mut() =
                    tokio_tungstenite::tungstenite::http::StatusCode::UNAUTHORIZED;
                Err(refused)
            }
        };
        tokio_tungstenite::accept_hdr_async(stream, check)
            .await
            .map_err(|_| ())
    }

    async fn receive(relay: &mut Relay) -> FromHost {
        loop {
            let message = tokio::time::timeout(Duration::from_secs(5), relay.next())
                .await
                .expect("nothing received from the host")
                .unwrap()
                .unwrap();
            if let Message::Text(text) = message {
                return serde_json::from_str(&text).unwrap();
            }
        }
    }

    async fn emit(relay: &mut Relay, message: &ToHost) {
        let text = serde_json::to_string(message).unwrap();
        relay.send(Message::Text(text.into())).await.unwrap();
    }

    fn start(
        addr: std::net::SocketAddr,
        key: &str,
    ) -> (Arc<Mutex<Vec<RelayEvent>>>, tokio::task::JoinHandle<()>) {
        let events: Arc<Mutex<Vec<RelayEvent>>> = Arc::default();
        let seen = Arc::clone(&events);
        let notify: Notify = Arc::new(move |e| seen.lock().unwrap().push(e));
        let config = RelayConfig {
            url: format!("ws://{addr}/host"),
            key: key.into(),
        };
        let core = Arc::new(Core::new(CODE.into(), Arc::default()));
        let task = tokio::spawn(run(
            config,
            CODE.into(),
            core,
            Arc::new(Fake::default()),
            notify,
        ));
        (events, task)
    }

    #[tokio::test]
    async fn answers_guests_pushes_state_and_resumes_after_a_cut() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (events, task) = start(listener.local_addr().unwrap(), KEY);

        let mut relay = accept(&listener).await.unwrap();
        assert_eq!(
            receive(&mut relay).await,
            FromHost::Hello {
                code: CODE.into(),
                resume: None
            }
        );
        let ready = ToHost::Ready {
            id: "j1".into(),
            secret: "s1".into(),
            url: "https://jam.example/j/j1/".into(),
            resumed: false,
        };
        emit(&mut relay, &ready).await;
        let join = ToHost::Request {
            rid: 7,
            client: "10.0.0.5".into(),
            path: "/api/join".into(),
            code: Some(CODE.into()),
            body: String::new(),
        };
        emit(&mut relay, &join).await;
        let (mut answered, mut state) = (false, false);
        while !(answered && state) {
            match receive(&mut relay).await {
                FromHost::Response { rid, status, body } => {
                    assert_eq!((rid, status), (7, 200));
                    assert_eq!(body, r#"{"ok":true}"#);
                    answered = true;
                }
                FromHost::State { data } => {
                    assert!(data.contains("Titre 7"));
                    state = true;
                }
                other => panic!("unexpected: {other:?}"),
            }
        }

        // Cut: the host comes back with the resume and keeps the same address.
        drop(relay);
        let mut relay = accept(&listener).await.unwrap();
        assert_eq!(
            receive(&mut relay).await,
            FromHost::Hello {
                code: CODE.into(),
                resume: Some(Resume {
                    id: "j1".into(),
                    secret: "s1".into()
                })
            }
        );
        emit(
            &mut relay,
            &ToHost::Ready {
                id: "j1".into(),
                secret: "s1".into(),
                url: "https://jam.example/j/j1/".into(),
                resumed: true,
            },
        )
        .await;
        // Let the host process `ready`.
        receive(&mut relay).await;
        task.abort();

        let events = events.lock().unwrap().clone();
        assert_eq!(events[0], RelayEvent::Connecting);
        assert!(matches!(
            &events[1],
            RelayEvent::Ready { resumed: false, .. }
        ));
        assert!(events.contains(&RelayEvent::Lost { reason: None }));
        assert!(matches!(
            events.last(),
            Some(RelayEvent::Ready { resumed: true, .. })
        ));
    }

    #[tokio::test]
    async fn a_refused_key_stops_the_online_jam() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (events, task) = start(listener.local_addr().unwrap(), "mauvaise");
        assert!(accept(&listener).await.is_err());
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("the online Jam should have stopped")
            .unwrap();
        let events = events.lock().unwrap().clone();
        assert!(matches!(events.last(), Some(RelayEvent::Failed { .. })));
    }

    #[test]
    fn protocol_matches_the_relay() {
        // Same messages as relay/src/protocol.rs (the relay's tests).
        let hello = serde_json::to_value(FromHost::Hello {
            code: CODE.into(),
            resume: None,
        })
        .unwrap();
        assert_eq!(
            hello,
            serde_json::json!({"type": "hello", "code": CODE, "resume": null})
        );
        let request: ToHost = serde_json::from_str(
            r#"{"type":"request","rid":1,"client":"1.2.3.4","path":"/api/add","code":null,"body":"{}"}"#,
        )
        .unwrap();
        assert!(matches!(request, ToHost::Request { rid: 1, .. }));
    }

    /// Jam opened on the real relay (key from `.env`), then called like a guest over
    /// HTTPS through Caddy: `cargo test live_relay -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore]
    async fn live_relay() {
        let config = RelayConfig::current()
            .expect("relay missing: SEAGLASS_RELAY_KEY and SEAGLASS_RELAY_BASE_URL from .env");
        let events: Arc<Mutex<Vec<RelayEvent>>> = Arc::default();
        let seen = Arc::clone(&events);
        let notify: Notify = Arc::new(move |e| seen.lock().unwrap().push(e));
        let core = Arc::new(Core::new(CODE.into(), Arc::default()));
        let task = tokio::spawn(run(
            config,
            CODE.into(),
            core,
            Arc::new(Fake::default()),
            notify,
        ));
        let url = loop {
            tokio::time::sleep(Duration::from_millis(200)).await;
            let last = events.lock().unwrap().last().cloned();
            match last {
                Some(RelayEvent::Ready { url, .. }) => break url,
                Some(RelayEvent::Failed { reason }) => panic!("refused: {reason}"),
                _ => {}
            }
        };
        println!("Online Jam: {url}");
        let http = reqwest::Client::new();
        let page = http.get(&url).send().await.unwrap();
        assert_eq!(page.status(), 200);
        assert!(page.text().await.unwrap().contains("jam.js"));

        let post = |call: &str, body: &str| {
            http.post(format!("{url}api/{call}"))
                .header("X-Jam-Code", CODE)
                .header("Content-Type", "application/json")
                .body(body.to_owned())
                .send()
        };
        assert_eq!(post("join", "").await.unwrap().status(), 200);
        let search = post("search", r#"{"query":"x"}"#).await.unwrap();
        assert_eq!(search.status(), 200);
        assert!(search.text().await.unwrap().contains("Titre 2 (Live)"));
        assert_eq!(
            http.post(format!("{url}api/join"))
                .header("X-Jam-Code", "ZZZZZZ")
                .send()
                .await
                .unwrap()
                .status(),
            401
        );

        // State stream through Caddy: the first event must arrive unbuffered.
        let mut events_stream = http
            .get(format!("{url}api/events?code={CODE}"))
            .send()
            .await
            .unwrap();
        assert_eq!(events_stream.status(), 200);
        let mut received = String::new();
        while !received.contains("data: ") {
            let chunk = tokio::time::timeout(Duration::from_secs(10), events_stream.chunk())
                .await
                .expect("no state received through Caddy")
                .unwrap()
                .expect("stream closed");
            received.push_str(&String::from_utf8_lossy(&chunk));
        }
        assert!(received.contains("Titre 7"));
        task.abort();
        println!("join, search, wrong code, state stream: ok");
    }
}
