//! Transfert de la lecture avec un ordinateur lié (#414), côté téléphone.
//!
//! Le PC sert les transferts sur le serveur de sa télécommande (`jam/remote`, réseau
//! local, « Accepter les transferts ») ; c'est toujours le téléphone qui agit :
//! « Reprendre ici » (`pull` : le PC donne sa lecture et se met en pause) et « Continuer
//! sur le PC » (`push` : le téléphone se met en pause et envoie la sienne). Le téléphone
//! n'écoute jamais sur le réseau.
//!
//! Le lien (adresse et code de la télécommande, `http://…:47801/#CODE`) est collé une fois
//! et gardé dans le gestionnaire de secrets (Keystore sur Android) : le code donne le
//! contrôle de la lecture du PC.
//!
//! Le même lien sert à synchroniser l'historique d'écoute (`history`, #430).

pub mod history;

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

use crate::i18n::{tr, trf};
use crate::jam::remote::is_code;
use crate::player::{Cmd, Handoff};

const KEYRING_SERVICE: &str = "seaglass";
const KEYRING_USER: &str = "handoff-link";
/// Réseau local : au-delà, le PC est éteint ou ailleurs.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(4);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

/// Ordinateur lié : adresse de sa télécommande et code de propriétaire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// `http://hôte:port/`.
    base: String,
    code: String,
}

/// Ce que l'UI montre de l'ordinateur lié (jamais le code).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkInfo {
    /// `192.168.1.105:47801`.
    pub host: String,
    /// Dernière synchro réussie de l'historique (ms, #430).
    pub synced_at: Option<i64>,
}

impl Link {
    /// Lien saisi par l'utilisateur : l'adresse de la télécommande avec son code après
    /// `#` (« Copier l'adresse » du PC), ou les deux tapés à la main, séparés par un espace
    /// (`192.168.1.20:47801 XXXX-XXXX-XXXX`, `http://` facultatif). Casse, espaces et
    /// tirets ignorés dans le code. `None` si ce n'en est pas un.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let text = if text.contains('#') {
            text.to_owned()
        } else {
            let (address, code) = text.split_once(char::is_whitespace)?;
            format!("{address}#{}", code.split_whitespace().collect::<String>())
        };
        let text = if text.contains("://") {
            text
        } else {
            format!("http://{text}")
        };
        let url = reqwest::Url::parse(&text).ok()?;
        if url.scheme() != "http" || !url.username().is_empty() || url.password().is_some() {
            return None;
        }
        let host = url.host_str()?;
        let code: String = url
            .fragment()?
            .bytes()
            .filter(|b| !b.is_ascii_whitespace() && *b != b'-')
            .map(|b| char::from(b.to_ascii_uppercase()))
            .collect();
        if !is_code(&code) {
            return None;
        }
        let base = match url.port() {
            Some(port) => format!("http://{host}:{port}/"),
            None => format!("http://{host}/"),
        };
        Some(Self { base, code })
    }

    pub fn info(&self) -> LinkInfo {
        let host = self
            .base
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .to_owned();
        LinkInfo {
            host,
            synced_at: None,
        }
    }

    /// Forme gardée dans le gestionnaire de secrets.
    fn stored(&self) -> String {
        format!("{}#{}", self.base, self.code)
    }

    /// Le PC répond et accepte le code (`join`).
    pub async fn check(&self) -> Result<(), String> {
        self.post("api/join", None).await.map(drop)
    }

    /// « Reprendre ici » : la lecture du PC, qui se met en pause.
    pub async fn take(&self) -> Result<Handoff, String> {
        let body = self.post("api/handoff/take", None).await?;
        serde_json::from_slice(&body).map_err(|e| {
            tracing::warn!("Transfert : réponse illisible ({e})");
            unreachable_pc()
        })
    }

    /// « Continuer sur le PC » : le PC reprend `handoff`.
    pub async fn give(&self, handoff: &Handoff) -> Result<(), String> {
        let body = serde_json::to_vec(handoff).map_err(|e| e.to_string())?;
        self.post("api/handoff/give", Some(body)).await.map(drop)
    }

    /// Un échange de l'historique d'écoute (#430).
    pub async fn sync_plays(
        &self,
        request: &history::SyncRequest,
    ) -> Result<history::SyncResponse, String> {
        let body = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        let body = self.post("api/plays/sync", Some(body)).await?;
        serde_json::from_slice(&body).map_err(|e| {
            tracing::warn!("Historique : réponse illisible ({e})");
            unreachable_pc()
        })
    }

    async fn post(&self, path: &str, body: Option<Vec<u8>>) -> Result<Vec<u8>, String> {
        let client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .no_proxy()
            .build()
            .map_err(|e| e.to_string())?;
        let mut request = client
            .post(format!("{}{path}", self.base))
            .header("X-Jam-Code", &self.code);
        if let Some(body) = body {
            request = request
                .header("Content-Type", "application/json")
                .body(body);
        }
        let response = request.send().await.map_err(|e| {
            tracing::info!("Transfert : PC injoignable ({e})");
            unreachable_pc()
        })?;
        let status = response.status().as_u16();
        let body = response.bytes().await.map_err(|_| unreachable_pc())?;
        if status == 200 {
            return Ok(body.to_vec());
        }
        let kind = serde_json::from_slice::<ErrorBody>(&body)
            .map(|b| b.error)
            .unwrap_or_default();
        tracing::info!("Transfert : refus du PC ({status} {kind})");
        Err(refusal(status, &kind))
    }
}

#[derive(Deserialize)]
struct ErrorBody {
    #[serde(default)]
    error: String,
}

fn unreachable_pc() -> String {
    tr(
        "ordinateur injoignable : il doit être allumé, Seaglass ouvert, sur le même réseau",
        "computer unreachable: it must be on, with Seaglass open, on the same network",
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

fn keyring_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|e| e.to_string())
}

/// Ordinateur lié, `None` s'il n'y en a pas (ou si le lien gardé est abîmé).
pub async fn load() -> Result<Option<Link>, String> {
    blocking(|| match keyring_entry()?.get_password() {
        Ok(stored) => Ok(Link::parse(&stored)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    })
    .await
}

pub async fn save(link: &Link) -> Result<(), String> {
    let stored = link.stored();
    blocking(move || {
        keyring_entry()?
            .set_password(&stored)
            .map_err(|e| e.to_string())
    })
    .await
}

pub async fn clear() -> Result<(), String> {
    blocking(|| match keyring_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    })
    .await
}

/// `keyring` est synchrone (et bloquant sur Android, appel au Keystore).
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

fn stopped() -> String {
    tr("lecteur arrêté", "player stopped").to_owned()
}

/// « Reprendre ici » : le PC donne sa lecture (et se met en pause), le lecteur d'ici la
/// reprend.
pub async fn pull(link: &Link, player: &mpsc::Sender<Cmd>) -> Result<(), String> {
    let handoff = link.take().await?;
    let handoff = handoff
        .received(crate::commands::settings::pins::is_uuid)
        .ok_or_else(|| tr("transfert illisible", "unreadable transfer").to_owned())?;
    let (tx, rx) = oneshot::channel();
    player
        .send(Cmd::ApplyHandoff(Box::new(handoff), tx))
        .await
        .map_err(|_| stopped())?;
    rx.await
        .map_err(|_| stopped())?
        .then_some(())
        .ok_or_else(stopped)
}

/// « Continuer sur le PC » : le lecteur d'ici se met en pause et donne sa lecture au PC.
/// Si le PC la refuse, la lecture reprend ici.
pub async fn push(link: &Link, player: &mpsc::Sender<Cmd>) -> Result<(), String> {
    let (tx, rx) = oneshot::channel();
    player
        .send(Cmd::GetState(tx))
        .await
        .map_err(|_| stopped())?;
    let was_playing = !rx.await.map_err(|_| stopped())?.paused;
    let (tx, rx) = oneshot::channel();
    player
        .send(Cmd::TakeHandoff(tx))
        .await
        .map_err(|_| stopped())?;
    let handoff = rx
        .await
        .map_err(|_| stopped())?
        .ok_or_else(|| tr("rien ne joue ici", "nothing is playing here").to_owned())?;
    if let Err(e) = link.give(&handoff).await {
        if was_playing {
            let _ = player.send(Cmd::SetPaused(false)).await;
        }
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODE: &str = "H4KX9QMT2WZP";

    #[test]
    fn parses_the_remote_address() {
        let link = Link::parse("http://192.168.1.105:47801/#H4KX9QMT2WZP").unwrap();
        assert_eq!(link.base, "http://192.168.1.105:47801/");
        assert_eq!(link.code, CODE);
        assert_eq!(link.info().host, "192.168.1.105:47801");
    }

    #[test]
    fn the_code_is_tolerant_but_checked() {
        let link = Link::parse("  http://192.168.1.105:47801/#h4kx-9qmt-2wzp \n").unwrap();
        assert_eq!(link.code, CODE);
        assert_eq!(Link::parse("http://192.168.1.105:47801/"), None);
        assert_eq!(Link::parse("http://192.168.1.105:47801/#AB23CD"), None);
        assert_eq!(
            Link::parse("http://192.168.1.105:47801/#H4KX9QMT2WZ0"),
            None
        );
    }

    #[test]
    fn only_plain_http_addresses() {
        assert_eq!(
            Link::parse("https://192.168.1.105:47801/#H4KX9QMT2WZP"),
            None
        );
        assert_eq!(Link::parse("ftp://192.168.1.105/#H4KX9QMT2WZP"), None);
        assert_eq!(Link::parse("http://a:b@192.168.1.105/#H4KX9QMT2WZP"), None);
        assert_eq!(Link::parse("H4KX9QMT2WZP"), None);
        // Un chemin ou une query sont ignorés : seule l'origine sert.
        let link = Link::parse("http://pc.local:47801/x?y=1#H4KX9QMT2WZP").unwrap();
        assert_eq!(link.base, "http://pc.local:47801/");
    }

    #[test]
    fn address_and_code_can_be_typed() {
        let expected = Link::parse("http://192.168.1.20:47801/#H4KX9QMT2WZP");
        for typed in [
            "192.168.1.20:47801 H4KX-9QMT-2WZP",
            "192.168.1.20:47801   h4kx 9qmt 2wzp",
            "http://192.168.1.20:47801/ H4KX9QMT2WZP",
            "192.168.1.20:47801#H4KX9QMT2WZP",
        ] {
            assert_eq!(Link::parse(typed), expected, "{typed}");
        }
        assert_eq!(Link::parse("192.168.1.20:47801"), None);
    }

    #[test]
    fn the_stored_form_round_trips() {
        let link = Link::parse("http://192.168.1.105:47801/#H4KX-9QMT-2WZP").unwrap();
        assert_eq!(Link::parse(&link.stored()), Some(link));
    }

    #[test]
    fn refusals_are_explained() {
        assert_ne!(refusal(403, "handoff"), refusal(403, "banned"));
        assert_ne!(refusal(409, "idle"), refusal(401, "code"));
        assert!(refusal(500, "backend").contains("500"));
    }
}
