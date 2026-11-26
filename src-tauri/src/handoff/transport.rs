//! Appels au PC lié : réseau local, puis relais chiffré, et messages de ses refus.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Deserialize;

use super::{Link, history, sealed};
use crate::i18n::{tr, trf};
use crate::jam::remote::RelayRoute;

/// Réseau local : au-delà, le PC est éteint ou ailleurs.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(4);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
/// Par le relais : réseau mobile, et le relais attend le PC jusqu'à 30 s.
const RELAY_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const RELAY_TIMEOUT: Duration = Duration::from_secs(45);

/// Le dernier appel n'a trouvé le PC que par le relais : les suivants y passent d'abord
/// (sinon chaque appel attendrait 4 s le réseau local), jusqu'au prochain lancement ou
/// jusqu'à ce que le relais ne réponde plus.
pub(super) static PREFER_RELAY: AtomicBool = AtomicBool::new(false);

impl Link {
    /// Réseau local, puis relais s'il est connu (ou l'inverse si le dernier appel n'a
    /// abouti que par le relais).
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
                        tracing::info!("Transfert : ordinateur joint par le relais");
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
            tracing::info!("Transfert : PC injoignable sur le réseau local ({e})");
            Unreachable
        })?;
        let status = response.status().as_u16();
        let body = response.bytes().await.map_err(|_| Unreachable)?;
        Ok((status, body.to_vec()))
    }

    /// Même appel, chiffré, par le tunnel du PC sur le relais.
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
                tracing::info!("Transfert : relais injoignable ({e})");
                Unreachable
            })?;
        let status = response.status().as_u16();
        let text = response.text().await.map_err(|_| Unreachable)?;
        match status {
            200 => {
                let answer =
                    sealed::open_response(&key, tunnel, &nonce, &text).ok_or_else(|| {
                        tracing::warn!("Transfert : réponse du relais illisible");
                        Unreachable
                    })?;
                Ok((answer.status, answer.body.into_bytes()))
            }
            // Clé refusée par le PC (« Nouveau code ») : même effet qu'un code refusé.
            403 => Ok((401, br#"{"error":"code"}"#.to_vec())),
            _ => {
                tracing::info!("Transfert : PC absent du relais ({status})");
                Err(Unreachable)
            }
        }
    }
}

/// Ni le réseau local ni le relais n'ont atteint le PC.
#[derive(Debug)]
pub(super) struct Unreachable;

/// Message d'une réponse d'erreur du PC.
pub(super) fn refused(status: u16, body: &[u8]) -> String {
    let kind = serde_json::from_slice::<ErrorBody>(body)
        .map(|b| b.error)
        .unwrap_or_default();
    tracing::info!("Transfert : refus du PC ({status} {kind})");
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

/// Message d'un refus du PC (code HTTP et type d'erreur de l'API de la télécommande).
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
