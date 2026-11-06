//! Jam en ligne (#25, étape 3b) : l'app se connecte au relais (`relay/`) en WebSocket
//! sortant, sans port ouvert chez l'hôte. Les appels des invités arrivent en `request`,
//! passent par le même cœur que la Jam locale (`api.rs`) et repartent en `response` ;
//! l'état est poussé au relais quand il change.
//!
//! Coupure (réseau mobile, veille) : reconnexion de plus en plus espacée (1 s à 30 s) avec
//! la reprise du relais, qui garde la même adresse et le même QR code 5 min. Au-delà, le
//! relais ouvre une nouvelle Jam : l'adresse change et l'UI en est prévenue.

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

/// Relais de production ; `SEAGLASS_RELAY_URL` au lancement en vise un autre (tests).
const DEFAULT_URL: &str = "wss://jam.bebanana.ch/host";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const READY_TIMEOUT: Duration = Duration::from_secs(10);
const PING_INTERVAL: Duration = Duration::from_secs(20);
/// Sans aucun message du relais pendant ce délai, la connexion est perdue.
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct RelayConfig {
    /// `wss://…/host` (ou `ws://` pour un relais local de test).
    pub url: String,
    pub key: String,
}

impl RelayConfig {
    /// Configuration compilée (`SEAGLASS_RELAY_KEY` du `.env`, lu par `build.rs`) ou donnée
    /// au lancement. `None` sans clé : seule la Jam locale est proposée.
    pub fn from_env() -> Option<Self> {
        let runtime = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        let key = runtime("SEAGLASS_RELAY_KEY")
            .or_else(|| option_env!("SEAGLASS_SEAGLASS_RELAY_KEY").map(str::to_owned))
            .filter(|k| !k.is_empty())?;
        let url = runtime("SEAGLASS_RELAY_URL").unwrap_or_else(|| DEFAULT_URL.to_owned());
        Some(Self { url, key })
    }
}

/// Ce que la connexion annonce à `JamState` (adresse, état pour l'UI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelayEvent {
    Connecting,
    /// Jam ouverte (ou reprise avec la même adresse si `resumed`).
    Ready {
        url: String,
        resumed: bool,
    },
    /// Connexion perdue ou refusée pour l'instant : nouvelle tentative.
    Lost {
        reason: Option<String>,
    },
    /// Refus définitif (clé refusée) : la Jam en ligne s'arrête.
    Failed {
        reason: String,
    },
}

pub type Notify = Arc<dyn Fn(RelayEvent) + Send + Sync>;

enum Failure {
    Retry(Option<String>),
    Fatal(String),
}

/// Tient la Jam ouverte sur le relais jusqu'à l'annulation de la tâche.
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
            // Connexion établie puis perdue : on repart vite.
            Ok(()) => {
                backoff = Duration::from_secs(1);
                notify(RelayEvent::Lost { reason: None });
            }
            Err(Failure::Retry(reason)) => notify(RelayEvent::Lost { reason }),
            Err(Failure::Fatal(reason)) => {
                tracing::warn!("Jam en ligne arrêtée : {reason}");
                notify(RelayEvent::Failed { reason });
                return;
            }
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

fn tls() -> Result<Arc<rustls::ClientConfig>, String> {
    use rustls_platform_verifier::BuilderVerifierExt;
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_platform_verifier()
        .map_err(|e| e.to_string())?
        .with_no_client_auth();
    Ok(Arc::new(config))
}

fn unreachable_relay() -> Failure {
    Failure::Retry(Some(
        tr("relais injoignable", "relay unreachable").to_owned(),
    ))
}

/// Une connexion : `Ok` si elle a été établie (Jam prête) puis perdue.
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
        Some(Connector::Rustls(tls().map_err(Failure::Fatal)?))
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
            tracing::info!("relais : {e}");
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
            tracing::info!(
                "Jam en ligne {}",
                if resumed { "reprise" } else { "ouverte" }
            );
            notify(RelayEvent::Ready { url, resumed });
        }
        // Relais complet, trop de Jams depuis cette adresse : réessayer plus tard.
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
                        // Jam fermée par le relais (12 h) : la reprise échouera, nouvelle Jam.
                        Ok(ToHost::Error { message }) => {
                            tracing::info!("relais : {message}");
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
// La réponse d'erreur du handshake de tungstenite est grosse : sans importance ici.
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

    /// Accepte un hôte en vérifiant sa clé, comme le relais.
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
                .expect("rien reçu de l'hôte")
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
                other => panic!("inattendu : {other:?}"),
            }
        }

        // Coupure : l'hôte revient avec la reprise et garde la même adresse.
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
        // Laisse l'hôte traiter `ready`.
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
            .expect("la Jam en ligne aurait dû s'arrêter")
            .unwrap();
        let events = events.lock().unwrap().clone();
        assert!(matches!(events.last(), Some(RelayEvent::Failed { .. })));
    }

    #[test]
    fn protocol_matches_the_relay() {
        // Mêmes messages que relay/src/protocol.rs (tests du relais).
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

    /// Jam ouverte sur le vrai relais (clé du `.env`), puis appelée comme un invité en
    /// HTTPS à travers Caddy : `cargo test live_relay -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore]
    async fn live_relay() {
        let config = RelayConfig::from_env().expect("SEAGLASS_RELAY_KEY absente du .env");
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
                Some(RelayEvent::Failed { reason }) => panic!("refusé : {reason}"),
                _ => {}
            }
        };
        println!("Jam en ligne : {url}");
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

        // Flux d'état à travers Caddy : le premier événement doit arriver sans tampon.
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
                .expect("aucun état reçu à travers Caddy")
                .unwrap()
                .expect("flux fermé");
            received.push_str(&String::from_utf8_lossy(&chunk));
        }
        assert!(received.contains("Titre 7"));
        task.abort();
        println!("rejoindre, recherche, code faux, flux d'état : ok");
    }
}
