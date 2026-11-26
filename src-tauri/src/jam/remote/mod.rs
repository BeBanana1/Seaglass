//! Remote control (#336): drive the PC's player from a phone's browser, on the local
//! network.
//!
//! Reuses the Jam's server (`server.rs`, HTTP, state stream, safeguards) on a port of its
//! own (47801, otherwise a free one): it runs without a Jam, and the Jam's server knows none
//! of its commands. The owner code (12 characters) is kept from one launch to the next
//! (`commands::remote`): the QR code is only scanned once. The app's side of the API is in
//! `backend.rs`. Desktop only, never through the online relay.

mod api;
mod backend;
mod tunnel;
mod view;

use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tokio::net::TcpListener;
use tokio::sync::{Notify, mpsc};
use tokio::task::JoinHandle;

use super::http::Response;
use super::server::{STYLE, Site, serve_site};
use super::{lan_ip, qr_svg};
use crate::i18n::{tr, trf};
use crate::player::Cmd;
use crate::store::Store;
use crate::tidal::TidalClient;
pub use api::{OWNER_CODE_LEN, PAIRING_WINDOW};
use api::{Owner, Remote};
use backend::AppRemote;
pub use tunnel::{RelayRoute, TunnelSettings};

/// Preferred port: the address stays the same from one launch to the next (bookmark).
const PORT: u16 = 47801;
const SEARCH_LIMIT: u32 = 30;

const PAGE: &str = include_str!("page/index.html");
const SCRIPT: &str = include_str!("page/remote.js");
const REMOTE_STYLE: &str = include_str!("page/remote.css");

/// New owner code.
pub fn new_code() -> String {
    super::guard::code_of_len(OWNER_CODE_LEN)
}

/// True if `code` is a valid owner code (read back from the settings).
pub fn is_code(code: &str) -> bool {
    super::guard::is_code(code, OWNER_CODE_LEN)
}

/// Running remote control, for Settings › Application.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStatus {
    /// Remote control page and commands served.
    pub remote: bool,
    /// Playback handoffs accepted (#414); the server runs if either is on.
    pub handoff: bool,
    pub url: String,
    pub code: String,
    /// False if no local network address was found (`127.0.0.1`).
    pub lan: bool,
    /// QR code of `url#code` as SVG (black on white).
    pub qr_svg: Option<String>,
    /// End of the last pairing window, ms since the Unix epoch (#62): a phone can learn
    /// the relay route until then. `None` before the first "Link a phone".
    pub pairing_until: Option<i64>,
    /// A relay is set up (#62): handoffs also work away from home, and "Link a phone" is
    /// worth offering. Read again each time the status is given (Settings › Relay).
    pub relay: bool,
}

/// Listening history served to the linked phone (#430).
pub struct History {
    pub store: Arc<Store>,
    /// Called with the number of listens received and added here.
    pub on_added: Box<dyn Fn(usize) + Send + Sync>,
}

struct Running {
    status: RemoteStatus,
    owner: Arc<Owner>,
    task: JoinHandle<()>,
    site: Arc<RemoteSite<AppRemote>>,
    /// Identity on the relay (#430); `None` without a built-in relay key.
    tunnel: Option<TunnelSettings>,
    /// Tunnel open while handoffs are accepted.
    tunnel_task: Option<JoinHandle<()>>,
}

impl Running {
    /// Opens or closes the relay tunnel according to "Accept handoffs".
    fn sync_tunnel(&mut self, handoff: bool) {
        match (handoff, self.tunnel_task.is_some()) {
            (true, false) => {
                let config = crate::jam::relay::RelayConfig::current();
                if let (Some(config), Some(settings)) = (config, self.tunnel.clone()) {
                    self.tunnel_task = Some(tokio::spawn(tunnel::run(
                        config,
                        settings,
                        Arc::clone(&self.site),
                    )));
                }
            }
            (false, true) => {
                if let Some(task) = self.tunnel_task.take() {
                    task.abort();
                }
                self.owner.set_relay(None);
            }
            _ => {}
        }
    }

    /// Status for the UI, with whether a relay is set up right now.
    fn view(&self) -> RemoteStatus {
        RemoteStatus {
            relay: crate::jam::relay::RelayConfig::current().is_some(),
            ..self.status.clone()
        }
    }

    /// New tunnel settings (key changed): the tunnel reopens with them, so the old key no
    /// longer opens anything. Unchanged settings keep the tunnel as it is.
    fn replace_tunnel(&mut self, tunnel: Option<TunnelSettings>) {
        if tunnel != self.tunnel {
            self.tunnel = tunnel;
            let handoff = self.status.handoff;
            self.sync_tunnel(false);
            self.sync_tunnel(handoff);
        }
    }

    fn stop(self) {
        self.task.abort();
        if let Some(task) = self.tunnel_task {
            task.abort();
        }
    }
}

#[derive(Default)]
pub struct RemoteState(Mutex<Option<Running>>);

impl RemoteState {
    pub fn status(&self) -> Option<RemoteStatus> {
        let slot = self.0.lock().ok()?;
        slot.as_ref().map(Running::view)
    }

    /// Serves the remote control and / or handoffs with `code`: starts the server, or changes
    /// what the running one serves. Both false stop it (`None`).
    pub async fn start(
        &self,
        code: String,
        tidal: Arc<TidalClient>,
        player: mpsc::Sender<Cmd>,
        history: Option<History>,
        tunnel: Option<TunnelSettings>,
        features: Features,
    ) -> Result<Option<RemoteStatus>, String> {
        if cfg!(mobile) {
            return Err(tr(
                "la télécommande n'existe que sur l'ordinateur",
                "the remote is only available on the computer",
            )
            .to_owned());
        }
        if !features.any() {
            self.stop();
            return Ok(None);
        }
        if let Some(status) = self.set_features(features) {
            return Ok(Some(status));
        }
        let (listener, url, lan) = listen().await?;
        let owner = Arc::new(Owner::new(code.clone()));
        owner.set_features(features.remote, features.handoff);
        let site = Arc::new(RemoteSite {
            owner: Arc::clone(&owner),
            remote: AppRemote {
                tidal,
                player,
                history: history.map(Arc::new),
            },
        });
        let task = tokio::spawn(serve_site(listener, Arc::clone(&site)));
        let status = RemoteStatus {
            remote: features.remote,
            handoff: features.handoff,
            qr_svg: qr_svg(&format!("{url}#{code}")),
            url,
            code,
            lan,
            pairing_until: None,
            relay: false,
        };
        let mut slot = self
            .0
            .lock()
            .map_err(|_| tr("télécommande indisponible", "remote unavailable").to_owned())?;
        // Two starts at the same time: the first one stays.
        if let Some(running) = slot.as_ref() {
            task.abort();
            return Ok(Some(running.view()));
        }
        tracing::info!("Remote control started");
        let mut running = Running {
            status,
            owner,
            task,
            site,
            tunnel,
            tunnel_task: None,
        };
        running.sync_tunnel(features.handoff);
        let view = running.view();
        *slot = Some(running);
        Ok(Some(view))
    }

    /// Changes what the running server serves; `None` if it is not running.
    fn set_features(&self, features: Features) -> Option<RemoteStatus> {
        let mut slot = self.0.lock().ok()?;
        let running = slot.as_mut()?;
        running
            .owner
            .set_features(features.remote, features.handoff);
        running.status.remote = features.remote;
        running.status.handoff = features.handoff;
        running.sync_tunnel(features.handoff);
        running.owner.refresh.notify_one();
        Some(running.view())
    }

    /// Replaces the running remote control's code: phones already paired will have to scan
    /// the new QR code.
    pub fn set_code(&self, code: String, tunnel: Option<TunnelSettings>) -> Option<RemoteStatus> {
        let mut slot = self.0.lock().ok()?;
        let running = slot.as_mut()?;
        running.owner.set_code(code.clone());
        running.replace_tunnel(tunnel);
        running.status.qr_svg = qr_svg(&format!("{}#{code}", running.status.url));
        running.status.code = code;
        Some(running.view())
    }

    /// "Link a phone" (#62): reopens the tunnel with `tunnel` (a new key) and opens the
    /// pairing window, during which a phone on the local network can learn the relay
    /// route. `None` if the server is not running.
    pub fn open_pairing(&self, tunnel: Option<TunnelSettings>) -> Option<RemoteStatus> {
        let mut slot = self.0.lock().ok()?;
        let running = slot.as_mut()?;
        running.replace_tunnel(tunnel);
        running.owner.open_pairing(PAIRING_WINDOW);
        let length = i64::try_from(PAIRING_WINDOW.as_millis()).unwrap_or(i64::MAX);
        running.status.pairing_until = Some(crate::jam::clock::now_ms() + length);
        Some(running.view())
    }

    /// Stops the remote control: port closed, pages cut off.
    pub fn stop(&self) {
        if let Some(running) = self.0.lock().ok().and_then(|mut s| s.take()) {
            running.stop();
            tracing::info!("Remote control stopped");
        }
    }
}

/// What the remote control's server serves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Features {
    /// Remote control page and commands (#336).
    pub remote: bool,
    /// Playback handoffs to and from the Android app (#414).
    pub handoff: bool,
}

impl Features {
    pub fn any(self) -> bool {
        self.remote || self.handoff
    }
}

/// Opens the remote control's port: listener, address to open, local address found.
async fn listen() -> Result<(TcpListener, String, bool), String> {
    let failed = |e: std::io::Error| {
        trf!(
            "télécommande impossible à démarrer : {}",
            "could not start the remote: {}",
            e
        )
    };
    let listener = match TcpListener::bind((Ipv4Addr::UNSPECIFIED, PORT)).await {
        Ok(l) => l,
        Err(_) => TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0))
            .await
            .map_err(failed)?,
    };
    let port = listener.local_addr().map_err(failed)?.port();
    let ip = lan_ip();
    let url = format!(
        "http://{}:{port}/",
        ip.unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
    );
    Ok((listener, url, ip.is_some()))
}

/// Remote control page and API, plugged into the shared server.
struct RemoteSite<R> {
    owner: Arc<Owner>,
    remote: R,
}

impl<R: Remote> Site for RemoteSite<R> {
    fn file(&self, path: &str) -> Option<Response> {
        if !self.owner.remote_enabled() {
            return None;
        }
        match path {
            "/" => Some(Response::new(200, "text/html; charset=utf-8", PAGE)),
            "/remote.js" => Some(Response::new(200, "text/javascript; charset=utf-8", SCRIPT)),
            // The Jam page's styles, then the remote control's own.
            "/jam.css" => Some(Response::new(200, "text/css; charset=utf-8", STYLE)),
            "/remote.css" => Some(Response::new(200, "text/css; charset=utf-8", REMOTE_STYLE)),
            _ => None,
        }
    }

    fn check_code(&self, ip: IpAddr, given: &str) -> Result<(), Response> {
        self.owner.check_code(ip, given)
    }

    async fn api(&self, path: &str, code: Option<&str>, body: &[u8], ip: IpAddr) -> Response {
        self.owner.api(path, code, body, ip, &self.remote).await
    }

    async fn state_json(&self) -> Option<String> {
        self.owner.state_json(&self.remote).await
    }

    fn refresh(&self) -> &Notify {
        &self.owner.refresh
    }

    fn large_body(&self, path: &str) -> bool {
        matches!(path, "/api/handoff/give" | api::HISTORY_PATH) && self.owner.handoff_enabled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_codes_are_long_and_checked() {
        let code = new_code();
        assert_eq!(code.len(), OWNER_CODE_LEN);
        assert!(is_code(&code));
        assert!(!is_code("AB23CD"));
        assert!(!is_code("AB23CD0O1IAB"));
    }
    // No test of `to_cmd`: dropping a `Cmd` in a test pulls in the notification area code,
    // and the Windows test binary no longer starts (`docs/android.md`).
}
