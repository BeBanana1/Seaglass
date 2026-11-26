//! Playback handoff with a linked computer (#414), phone side.
//!
//! The PC serves handoffs on its remote control server (`jam/remote`, local network,
//! "Accept handoffs"); the phone always acts: "Resume here" (`pull`: the PC gives its
//! playback and pauses) and "Continue on the PC" (`push`: the phone pauses and sends its
//! own). The phone never listens on the network.
//!
//! The link (address and code of the remote control, `http://…:47801/#CODE`) is pasted once
//! and kept in the credential store (`credentials.rs`, Keystore on Android): the code gives
//! control of the PC's playback.
//!
//! The same link syncs the listening history (`history`, #430). Off the local network, both
//! go through the relay, end-to-end encrypted (`sealed`, `transport.rs`): the online address
//! and the key are learned from the PC on the local network and kept with the link.

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

/// Linked computer: address of its remote control and owner code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// `http://host:port/`.
    base: String,
    code: String,
    /// Route through the relay (#430), learned on the local network.
    relay: Option<RelayRoute>,
}

/// What the UI shows of the linked computer (never the code).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkInfo {
    /// `192.168.1.105:47801`.
    pub host: String,
    /// Last successful history sync (ms, #430).
    pub synced_at: Option<i64>,
    /// Also reachable through the relay, off the local network (#430).
    pub online: bool,
}

impl Link {
    /// Link typed by the user: the remote control's address with its code after `#` (the
    /// PC's "Copy address"), or both typed by hand, separated by a space
    /// (`192.168.1.20:47801 XXXX-XXXX-XXXX`, `http://` optional). Case, spaces and dashes are
    /// ignored in the code. `None` if it is not one.
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

    /// Form kept in the credential store.
    fn stored(&self) -> String {
        format!("{}#{}", self.base, self.code)
    }

    /// The PC answers and accepts the code (`join`).
    pub async fn check(&self) -> Result<(), String> {
        self.post("api/join", None).await.map(drop)
    }

    /// "Resume here": the PC's playback, and the PC pauses.
    pub async fn take(&self) -> Result<Handoff, String> {
        let body = self.post("api/handoff/take", None).await?;
        serde_json::from_slice(&body).map_err(|e| {
            tracing::warn!("Handoff: unreadable answer ({e})");
            unreachable_pc()
        })
    }

    /// "Continue on the PC": the PC resumes `handoff`.
    pub async fn give(&self, handoff: &Handoff) -> Result<(), String> {
        let body = serde_json::to_vec(handoff).map_err(|e| e.to_string())?;
        self.post("api/handoff/give", Some(body)).await.map(drop)
    }

    /// One exchange of the listening history (#430).
    pub async fn sync_plays(
        &self,
        request: &history::SyncRequest,
    ) -> Result<history::SyncResponse, String> {
        let body = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        let body = self.post("api/plays/sync", Some(body)).await?;
        serde_json::from_slice(&body).map_err(|e| {
            tracing::warn!("History: unreadable answer ({e})");
            unreachable_pc()
        })
    }

    /// Route through the relay known to the phone.
    pub fn relay(&self) -> Option<&RelayRoute> {
        self.relay.as_ref()
    }

    pub fn with_relay(self, relay: Option<RelayRoute>) -> Self {
        Self { relay, ..self }
    }

    /// Asks the PC, **on the local network only**, how to reach it through the relay
    /// (#430). `None` if it has no open tunnel (no relay key, relay not up to date) or if it
    /// is too old.
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

/// "Resume here": the PC gives its playback (and pauses), the player here resumes it.
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

/// "Continue on the PC": the player here pauses and gives its playback to the PC. If the
/// PC refuses it, playback resumes here.
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
        // A path or a query is ignored: only the origin counts.
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
