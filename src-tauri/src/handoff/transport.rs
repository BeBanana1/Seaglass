//! Calls to the linked PC: local network, then the encrypted relay, and messages for its
//! refusals.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Deserialize;

use super::{Link, history, sealed};
use crate::i18n::{tr, trf};
use crate::jam::remote::RelayRoute;

/// Local network: past this, the PC is off or elsewhere.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(4);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
/// Through the relay: mobile network, and the relay waits up to 30 s for the PC.
const RELAY_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const RELAY_TIMEOUT: Duration = Duration::from_secs(45);

/// The last call only reached the PC through the relay: the next ones try it first
/// (otherwise each call would wait 4 s for the local network), until the next launch or
/// until the relay stops answering.
pub(super) static PREFER_RELAY: AtomicBool = AtomicBool::new(false);

impl Link {
    /// Local network, then the relay if it is known (or the other way round if the last
    /// call only went through the relay).
    pub(super) async fn post(&self, path: &str, body: Option<Vec<u8>>) -> Result<Vec<u8>, String> {
        let answer = match &self.relay {
            None => self.post_lan(path, body).await,
            Some(route) if PREFER_RELAY.load(Ordering::Relaxed) => {
                match self.post_relay(route, path, body.as_deref()).await {
                    Err(Unreachable) => self.post_lan(path, body).await.inspect(|_| {
                        PREFER_RELAY.store(false, Ordering::Relaxed);
                    }),
                    answer => answer,
                }
            }
            Some(route) => match self.post_lan(path, body.clone()).await {
                Err(Unreachable) => {
                    let answer = self.post_relay(route, path, body.as_deref()).await;
                    if answer.is_ok() {
                        tracing::info!("Handoff: computer reached through the relay");
                        PREFER_RELAY.store(true, Ordering::Relaxed);
                    }
                    answer
                }
                answer => answer,
            },
        };
        let (status, body) = answer.map_err(|_| unreachable_pc())?;
        if status == 200 {
            return Ok(body);
        }
        Err(refused(status, &body))
    }

    pub(super) async fn post_lan(
        &self,
        path: &str,
        body: Option<Vec<u8>>,
    ) -> Result<(u16, Vec<u8>), Unreachable> {
        let client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .no_proxy()
            .build()
            .map_err(|_| Unreachable)?;
        let mut request = client
            .post(format!("{}{path}", self.base))
            .header("X-Jam-Code", &self.code);
        if let Some(body) = body {
            request = request
                .header("Content-Type", "application/json")
                .body(body);
        }
        let response = request.send().await.map_err(|e| {
            tracing::info!("Handoff: PC unreachable on the local network ({e})");
            Unreachable
        })?;
        let status = response.status().as_u16();
        let body = response.bytes().await.map_err(|_| Unreachable)?;
        Ok((status, body.to_vec()))
    }

    /// Same call, encrypted, through the PC's tunnel on the relay.
    async fn post_relay(
        &self,
        route: &RelayRoute,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<(u16, Vec<u8>), Unreachable> {
        let key = sealed::TunnelKey::parse(&route.key).ok_or(Unreachable)?;
        let tunnel = route.url.rsplit('/').next().unwrap_or_default();
        let request = sealed::Request {
            at: history::now_ms(),
            path: format!("/{path}"),
            code: self.code.clone(),
            body: String::from_utf8_lossy(body.unwrap_or_default()).into_owned(),
        };
        let (packet, nonce) = sealed::seal_request(&key, tunnel, &request);
        let client = reqwest::Client::builder()
            .connect_timeout(RELAY_CONNECT_TIMEOUT)
            .timeout(RELAY_TIMEOUT)
            .build()
            .map_err(|_| Unreachable)?;
        let response = client
            .post(&route.url)
            .header("Content-Type", "text/plain")
            .body(packet)
            .send()
            .await
            .map_err(|e| {
                tracing::info!("Handoff: relay unreachable ({e})");
                Unreachable
            })?;
        let status = response.status().as_u16();
        let text = response.text().await.map_err(|_| Unreachable)?;
        match status {
            200 => {
                let answer =
                    sealed::open_response(&key, tunnel, &nonce, &text).ok_or_else(|| {
                        tracing::warn!("Handoff: unreadable answer from the relay");
                        Unreachable
                    })?;
                Ok((answer.status, answer.body.into_bytes()))
            }
            // Key refused by the PC ("New code"): same effect as a refused code.
            403 => Ok((401, br#"{"error":"code"}"#.to_vec())),
            _ => {
                tracing::info!("Handoff: PC not on the relay ({status})");
                Err(Unreachable)
            }
        }
    }
}

/// Neither the local network nor the relay reached the PC.
#[derive(Debug)]
pub(super) struct Unreachable;

/// Message for an error answer from the PC.
pub(super) fn refused(status: u16, body: &[u8]) -> String {
    let kind = serde_json::from_slice::<ErrorBody>(body)
        .map(|b| b.error)
        .unwrap_or_default();
    tracing::info!("Handoff: refused by the PC ({status} {kind})");
    refusal(status, &kind)
}

#[derive(Deserialize)]
struct ErrorBody {
    #[serde(default)]
    error: String,
}

pub(super) fn unreachable_pc() -> String {
    tr(
        "ordinateur injoignable : il doit être allumé avec Seaglass ouvert (et sur le même réseau tant qu'il n'y a pas été joint une fois)",
        "computer unreachable: it must be on with Seaglass open (and on the same network until it has been reached there once)",
    )
    .to_owned()
}

/// Message for a refusal by the PC (HTTP status and error kind of the remote control API).
fn refusal(status: u16, kind: &str) -> String {
    match (status, kind) {
        (401, _) => tr(
            "code refusé : liez de nouveau l'ordinateur",
            "code refused: link the computer again",
        ),
        (403, "handoff") => tr(
            "l'ordinateur n'accepte pas les transferts (Réglages › Application)",
            "the computer does not accept transfers (Settings › App)",
        ),
        (409, "idle") => tr(
            "rien ne joue sur l'ordinateur",
            "nothing is playing on the computer",
        ),
        (429, _) => tr(
            "trop d'essais, réessayez plus tard",
            "too many attempts, try again later",
        ),
        (404, _) => tr(
            "cet ordinateur ne connaît pas les transferts : mettez Seaglass à jour",
            "this computer does not support transfers: update Seaglass",
        ),
        _ => return trf!("transfert refusé ({})", "transfer refused ({})", status),
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::refusal;

    #[test]
    fn refusals_are_explained() {
        assert_ne!(refusal(403, "handoff"), refusal(403, "banned"));
        assert_ne!(refusal(409, "idle"), refusal(401, "code"));
        assert!(refusal(500, "backend").contains("500"));
    }
}
