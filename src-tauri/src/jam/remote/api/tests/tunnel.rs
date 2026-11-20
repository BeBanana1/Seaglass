//! Tunnel du relais (#430) de bout en bout : le PC (faux lecteur, vrai historique en
//! mémoire) branché sur un faux relais, et le client du téléphone qui ne joint pas le
//! réseau local et passe par le relais, chiffré.

use std::collections::HashMap;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

use super::handoff::sample;
use super::*;
use crate::handoff::history::sync;
use crate::jam::protocol::{FromDevice, ToDevice};
use crate::jam::relay::RelayConfig;
use crate::jam::remote::tunnel;
use crate::store::{Play, Store};

const KEY: &str = "cle-de-test";

type Forward = (String, oneshot::Sender<(u16, String)>);

/// Faux relais : un PC en WebSocket (`/device`), les appels du téléphone en HTTP
/// (`POST /d/<id>`) transmis tels quels. Renvoie l'adresse et l'URL du tunnel.
async fn fake_relay() -> (SocketAddr, oneshot::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (ready_tx, ready_rx) = oneshot::channel();
    let (forward_tx, mut forward_rx) = mpsc::channel::<Forward>(8);
    tokio::spawn(async move {
        let mut ready_tx = Some(ready_tx);
        let mut device = None;
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let mut head = [0u8; 4];
            stream.peek(&mut head).await.unwrap();
            if &head == b"GET " && device.is_none() {
                let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
                let Some(Ok(Message::Text(text))) = socket.next().await else {
                    panic!("pas de hello");
                };
                let FromDevice::Hello { id, .. } = serde_json::from_str(&text).unwrap() else {
                    panic!("pas de hello");
                };
                let url = format!("http://{addr}/d/{id}");
                let ready = ToDevice::Ready { url: url.clone() };
                let text = serde_json::to_string(&ready).unwrap();
                socket.send(Message::Text(text.into())).await.unwrap();
                if let Some(tx) = ready_tx.take() {
                    let _ = tx.send(url);
                }
                let mut forward_rx = std::mem::replace(&mut forward_rx, mpsc::channel(1).1);
                device = Some(tokio::spawn(async move {
                    let mut pending: HashMap<u64, oneshot::Sender<(u16, String)>> = HashMap::new();
                    let mut rid = 0;
                    loop {
                        tokio::select! {
                            Some((body, reply)) = forward_rx.recv() => {
                                rid += 1;
                                pending.insert(rid, reply);
                                let request = ToDevice::Request { rid, client: "127.0.0.1".into(), body };
                                let text = serde_json::to_string(&request).unwrap();
                                socket.send(Message::Text(text.into())).await.unwrap();
                            }
                            Some(Ok(message)) = socket.next() => {
                                if let Message::Text(text) = message
                                    && let Ok(FromDevice::Response { rid, status, body }) =
                                        serde_json::from_str(&text)
                                    && let Some(reply) = pending.remove(&rid)
                                {
                                    let _ = reply.send((status, body));
                                }
                            }
                        }
                    }
                }));
                continue;
            }
            let forward = forward_tx.clone();
            tokio::spawn(async move { phone_call(stream, forward).await });
        }
    });
    (addr, ready_rx)
}

/// Un appel HTTP du téléphone : corps transmis au PC, réponse renvoyée.
async fn phone_call(stream: TcpStream, forward: mpsc::Sender<Forward>) {
    let mut reader = BufReader::new(stream);
    let mut length = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        if line == "\r\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap();
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).await.unwrap();
    let (tx, rx) = oneshot::channel();
    forward
        .send((String::from_utf8(body).unwrap(), tx))
        .await
        .unwrap();
    let (status, body) = rx.await.unwrap();
    let response = format!(
        "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    reader
        .into_inner()
        .write_all(response.as_bytes())
        .await
        .unwrap();
}

fn listen(store: &Store, id: u64, at: i64) {
    store
        .add_play(&Play {
            track: &track(id),
            album: &album(id),
            source: None,
            played_at: at,
            format: None,
        })
        .unwrap();
}

#[tokio::test]
async fn the_phone_reaches_the_computer_through_the_relay() {
    let (lan, site) = start().await;
    site.owner.set_features(false, true);
    // Avant le tunnel, le PC n'a pas de chemin en ligne à donner.
    assert_eq!(post(lan, "/api/relay", "").await.0, 404);

    let (relay, ready) = fake_relay().await;
    let settings = tunnel::TunnelSettings::new();
    let config = RelayConfig {
        url: format!("ws://{relay}/host"),
        key: KEY.into(),
    };
    tokio::spawn(tunnel::run(config, settings.clone(), Arc::clone(&site)));
    let url = tokio::time::timeout(Duration::from_secs(5), ready)
        .await
        .unwrap()
        .unwrap();
    // Le téléphone apprend le chemin sur le réseau local.
    let mut route = None;
    for _ in 0..50 {
        let linked = crate::handoff::Link::parse(&format!("http://{lan}/#{CODE}")).unwrap();
        route = linked.fetch_relay().await.unwrap();
        if route.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let route = route.expect("chemin par le relais");
    assert_eq!(route.url, url);
    assert_eq!(route.key, settings.key);

    // Hors du réseau local (port fermé) : tout passe par le relais.
    let away = crate::handoff::Link::parse(&format!("http://127.0.0.1:1/#{CODE}"))
        .unwrap()
        .with_relay(Some(route.clone()));
    away.check().await.unwrap();
    assert_eq!(away.take().await.unwrap(), sample(200));

    let pc = Arc::clone(site.remote.history());
    for i in 0..1_500 {
        listen(&pc, i, 1_000 + i as i64);
    }
    let phone = Arc::new(Store::in_memory().unwrap());
    listen(&phone, 9_999, 5);
    assert_eq!(sync(&away, &phone).await.unwrap(), 1_500);
    assert_eq!(
        pc.played_track_ids().unwrap(),
        phone.played_track_ids().unwrap()
    );

    // Une clé changée (« Nouveau code ») : le PC refuse, message de code refusé.
    let stale = crate::handoff::Link::parse(&format!("http://127.0.0.1:1/#{CODE}"))
        .unwrap()
        .with_relay(Some(tunnel::RelayRoute {
            url: route.url.clone(),
            key: crate::handoff::sealed::new_key(),
        }));
    let refused = stale.check().await.unwrap_err();
    assert!(refused.contains("code"), "{refused}");
}
