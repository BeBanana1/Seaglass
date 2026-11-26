//! Télécommande (#336) : piloter le lecteur du PC depuis le navigateur d'un téléphone, sur
//! le réseau local.
//!
//! Réutilise le serveur de la Jam (`server.rs`, HTTP, flux d'état, garde-fous) sur un port
//! à part (47801, sinon un port libre) : elle tourne sans Jam, et le serveur de la Jam ne
//! connaît aucune de ses commandes. Le code de propriétaire (12 caractères) est gardé
//! d'un lancement à l'autre (`commands::remote`) : le QR code ne se scanne qu'une fois.
//! Bureau seulement, jamais par le relais en ligne.

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

/// Port préféré : l'adresse reste la même d'un lancement à l'autre (marque-page).
const PORT: u16 = 47801;
const SEARCH_LIMIT: u32 = 30;

const PAGE: &str = include_str!("page/index.html");
const SCRIPT: &str = include_str!("page/remote.js");
const REMOTE_STYLE: &str = include_str!("page/remote.css");

/// Nouveau code de propriétaire.
pub fn new_code() -> String {
    super::guard::code_of_len(OWNER_CODE_LEN)
}

/// Vrai si `code` est un code de propriétaire valable (relu dans les réglages).
pub fn is_code(code: &str) -> bool {
    super::guard::is_code(code, OWNER_CODE_LEN)
}

/// Télécommande en cours, pour Réglages › Application.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStatus {
    /// Page et commandes de la télécommande servies.
    pub remote: bool,
    /// Transferts de lecture acceptés (#414) ; le serveur tourne si l'un des deux l'est.
    pub handoff: bool,
    pub url: String,
    pub code: String,
    /// Faux si aucune adresse du réseau local n'a été trouvée (`127.0.0.1`).
    pub lan: bool,
    /// QR code de `url#code` en SVG (noir sur blanc).
    pub qr_svg: Option<String>,
    /// End of the last pairing window, ms since the Unix epoch (#62): a phone can learn
    /// the relay route until then. `None` before the first "Link a phone".
    pub pairing_until: Option<i64>,
    /// A relay is set up (#62): handoffs also work away from home, and "Link a phone" is
    /// worth offering. Read again each time the status is given (Settings › Relay).
    pub relay: bool,
}

/// Historique d'écoute servi au téléphone lié (#430).
pub struct History {
    pub store: Arc<Store>,
    /// Appelé avec le nombre d'écoutes reçues et ajoutées ici.
    pub on_added: Box<dyn Fn(usize) + Send + Sync>,
}

struct Running {
    status: RemoteStatus,
    owner: Arc<Owner>,
    task: JoinHandle<()>,
    site: Arc<RemoteSite<AppRemote>>,
    /// Identité sur le relais (#430) ; `None` sans clé de relais compilée.
    tunnel: Option<TunnelSettings>,
    /// Tunnel ouvert tant que les transferts sont acceptés.
    tunnel_task: Option<JoinHandle<()>>,
}

impl Running {
    /// Ouvre ou ferme le tunnel du relais selon « Accepter les transferts ».
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

    /// Sert la télécommande et / ou les transferts avec `code` : démarre le serveur, ou
    /// change ce que sert celui qui tourne déjà. Les deux à faux l'arrêtent (`None`).
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
        // Deux démarrages simultanés : le premier arrivé reste.
        if let Some(running) = slot.as_ref() {
            task.abort();
            return Ok(Some(running.view()));
        }
        tracing::info!("Télécommande démarrée");
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

    /// Change ce que sert le serveur en cours ; `None` s'il ne tourne pas.
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

    /// Remplace le code de la télécommande en cours : les téléphones déjà appairés devront
    /// scanner le nouveau QR code.
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

    /// Arrête la télécommande : port fermé, pages coupées.
    pub fn stop(&self) {
        if let Some(running) = self.0.lock().ok().and_then(|mut s| s.take()) {
            running.stop();
            tracing::info!("Télécommande arrêtée");
        }
    }
}

/// Ce que sert le serveur de la télécommande.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Features {
    /// Page et commandes de la télécommande (#336).
    pub remote: bool,
    /// Transferts de lecture vers et depuis l'app Android (#414).
    pub handoff: bool,
}

impl Features {
    pub fn any(self) -> bool {
        self.remote || self.handoff
    }
}

/// Ouvre le port de la télécommande : écouteur, adresse à ouvrir, adresse locale trouvée.
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

/// Page et API de la télécommande, branchées sur le serveur commun.
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
            // Styles de la page de la Jam, puis ceux propres à la télécommande.
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
    // Pas de test de `to_cmd` : détruire un `Cmd` dans un test tire le code de la zone de
    // notification, et le binaire de test Windows ne démarre plus (`docs/android.md`).
}
