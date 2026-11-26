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
//! Le même lien sert à synchroniser l'historique d'écoute (`history`, #430). Hors du réseau
//! local, les deux passent par le relais, chiffrés de bout en bout (`sealed`) : l'adresse
//! en ligne et la clé sont apprises du PC sur le réseau local et gardées avec le lien.

pub mod history;
pub mod sealed;

mod credentials;
mod transport;

pub use credentials::{clear, load, save, save_relay};
use transport::{refused, unreachable_pc};

use serde::Serialize;
use tokio::sync::{mpsc, oneshot};

use crate::i18n::tr;
use crate::jam::remote::{RelayRoute, is_code};
use crate::player::{Cmd, Handoff};

/// Ordinateur lié : adresse de sa télécommande et code de propriétaire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// `http://hôte:port/`.
    base: String,
    code: String,
    /// Chemin par le relais (#430), appris sur le réseau local.
    relay: Option<RelayRoute>,
}

/// Ce que l'UI montre de l'ordinateur lié (jamais le code).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkInfo {
    /// `192.168.1.105:47801`.
    pub host: String,
    /// Dernière synchro réussie de l'historique (ms, #430).
    pub synced_at: Option<i64>,
    /// Joignable aussi par le relais, hors du réseau local (#430).
    pub online: bool,
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
        Some(Self {
            base,
            code,
            relay: None,
        })
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
            online: self.relay.is_some(),
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

    /// Chemin par le relais connu du téléphone.
    pub fn relay(&self) -> Option<&RelayRoute> {
        self.relay.as_ref()
    }

    pub fn with_relay(self, relay: Option<RelayRoute>) -> Self {
        Self { relay, ..self }
    }

    /// Demande au PC, **sur le réseau local seulement**, de quoi le joindre par le relais
    /// (#430). `None` s'il n'a pas de tunnel ouvert (pas de clé de relais, relais pas à
    /// jour) ou s'il est trop ancien.
    pub async fn fetch_relay(&self) -> Result<Option<RelayRoute>, String> {
        let (status, body) = self
            .post_lan("api/relay", None)
            .await
            .map_err(|_| unreachable_pc())?;
        match status {
            200 => Ok(serde_json::from_slice::<RelayRoute>(&body)
                .ok()
                .filter(|r| sealed::TunnelKey::parse(&r.key).is_some())),
            404 => Ok(None),
            _ => Err(refused(status, &body)),
        }
    }
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
}
