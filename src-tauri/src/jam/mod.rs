//! Jam (#25) : des invités ajoutent des titres à la file de l'hôte depuis une page web,
//! sans app ni compte. Seule la machine de l'hôte lit.
//!
//! Deux modes, mêmes règles (`api.rs`) :
//! - **En ligne** (`relay.rs`) : l'app se connecte au relais (`relay/`), les invités ouvrent
//!   `https://jam.bebanana.ch/j/<id>/`, sans Wi-Fi commun.
//! - **Réseau local** (`server.rs`) : l'app ouvre un port (47800, sinon un port libre).
//!   Bureau seulement : sur un téléphone en données mobiles (ou qui partage sa connexion),
//!   l'adresse ne servirait à rien, et l'app n'ouvre aucun port sur Android.
//!
//! Le code d'accès est tiré pour chaque Jam ; arrêter rend le code invalide. Le QR code
//! porte l'adresse et le code (après `#`, jamais envoyé au serveur ni journalisé).

mod api;
mod guard;
mod guests;
mod http;
mod protocol;
mod relay;
mod server;
#[cfg(test)]
mod testing;

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::sync::{Arc, Mutex};

use qrcode::QrCode;
use qrcode::render::svg;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::i18n::{tr, trf};
use crate::player::Cmd;
use crate::tidal::TidalClient;
use crate::tidal::models::{Track, TrackId};
use api::{Backend, Core, Snapshot};
use guests::Guests;
use relay::{RelayConfig, RelayEvent};

/// Port préféré : l'adresse reste la même d'une Jam à l'autre (marque-page des invités).
const PORT: u16 = 47800;
const SEARCH_LIMIT: u32 = 30;
/// Événement vers l'UI de l'hôte : `{ <id du titre>: <nom de l'invité> }`.
const GUESTS_EVENT: &str = "jam-guests";
/// Événement vers l'UI de l'hôte : `JamStatus` (connexion au relais, adresse changée).
const STATUS_EVENT: &str = "jam-status";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JamMode {
    Online,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Connection {
    Connecting,
    Online,
    Reconnecting,
    /// Refus définitif du relais (clé) : la Jam reste affichée jusqu'à « Arrêter ».
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JamInfo {
    pub mode: JamMode,
    /// Adresse à donner aux invités ; `None` tant que le relais n'a pas répondu.
    pub url: Option<String>,
    pub code: String,
    /// Réseau local : faux si aucune adresse du réseau n'a été trouvée (`127.0.0.1`).
    pub lan: bool,
    /// QR code de `url#code` en SVG (noir sur blanc), `None` sans adresse.
    pub qr_svg: Option<String>,
    pub connection: Connection,
    /// Raison d'une reconnexion ou d'un échec, traduite.
    pub reason: Option<String>,
}

/// État de la Jam pour l'UI de l'hôte.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JamStatus {
    #[serde(flatten)]
    pub info: JamInfo,
    /// Nom de l'invité de chaque titre ajouté par la Jam.
    pub guests: HashMap<u64, String>,
}

struct Running {
    info: Arc<Mutex<JamInfo>>,
    guests: Arc<Mutex<Guests>>,
    task: JoinHandle<()>,
}

fn status_of(info: &Mutex<JamInfo>, guests: &Mutex<Guests>) -> Option<JamStatus> {
    Some(JamStatus {
        info: info.lock().ok()?.clone(),
        guests: guests.lock().map(|g| g.snapshot()).unwrap_or_default(),
    })
}

/// Vrai si l'app a une clé de relais : la Jam en ligne est proposée.
pub fn online_available() -> bool {
    RelayConfig::from_env().is_some()
}

#[derive(Default)]
pub struct JamState(Mutex<Option<Running>>);

impl JamState {
    pub fn status(&self) -> Option<JamStatus> {
        let slot = self.0.lock().ok()?;
        let running = slot.as_ref()?;
        status_of(&running.info, &running.guests)
    }

    /// Démarre la Jam, ou renvoie celle qui tourne déjà.
    pub async fn start(
        &self,
        mode: JamMode,
        app: AppHandle,
        tidal: Arc<TidalClient>,
        player: mpsc::Sender<Cmd>,
    ) -> Result<JamStatus, String> {
        if let Some(status) = self.status() {
            return Ok(status);
        }
        let code = guard::new_code();
        let guests: Arc<Mutex<Guests>> = Arc::default();
        let core = Arc::new(Core::new(code.clone(), Arc::clone(&guests)));
        let backend = Arc::new(AppBackend {
            app: app.clone(),
            tidal,
            player,
        });
        let (info, task) = match mode {
            JamMode::Local if cfg!(mobile) => {
                return Err(tr(
                    "la Jam sur le réseau local n'existe que sur l'ordinateur",
                    "the local network Jam is only available on the computer",
                )
                .to_owned());
            }
            JamMode::Local => {
                let (listener, url, lan) = listen().await?;
                let info = JamInfo {
                    mode,
                    qr_svg: qr_svg(&format!("{url}#{code}")),
                    url: Some(url),
                    code: code.clone(),
                    lan,
                    connection: Connection::Online,
                    reason: None,
                };
                let info = Arc::new(Mutex::new(info));
                (info, tokio::spawn(server::serve(listener, core, backend)))
            }
            JamMode::Online => {
                let config = RelayConfig::from_env().ok_or_else(|| {
                    tr(
                        "Jam en ligne indisponible : relais non configuré",
                        "online Jam unavailable: no relay configured",
                    )
                    .to_owned()
                })?;
                let info = Arc::new(Mutex::new(JamInfo {
                    mode,
                    url: None,
                    code: code.clone(),
                    lan: true,
                    qr_svg: None,
                    connection: Connection::Connecting,
                    reason: None,
                }));
                let notify = relay_notify(app, Arc::clone(&info), Arc::clone(&guests));
                let task = tokio::spawn(relay::run(config, code, core, backend, notify));
                (info, task)
            }
        };
        let mut slot = self
            .0
            .lock()
            .map_err(|_| tr("Jam indisponible", "Jam unavailable").to_owned())?;
        // Deux démarrages simultanés : le premier arrivé reste.
        if slot.is_some() {
            task.abort();
            drop(slot);
            return self
                .status()
                .ok_or_else(|| tr("Jam indisponible", "Jam unavailable").to_owned());
        }
        tracing::info!("Jam démarrée ({mode:?})");
        let status = status_of(&info, &guests);
        *slot = Some(Running { info, guests, task });
        status.ok_or_else(|| tr("Jam indisponible", "Jam unavailable").to_owned())
    }

    /// Arrête la Jam : port fermé ou connexion au relais coupée, code invalide.
    pub fn stop(&self) {
        if let Some(running) = self.0.lock().ok().and_then(|mut s| s.take()) {
            running.task.abort();
            tracing::info!("Jam arrêtée");
        }
    }
}

/// Ouvre le port de la Jam locale : écouteur, adresse à donner, adresse locale trouvée.
async fn listen() -> Result<(TcpListener, String, bool), String> {
    let failed = |e: std::io::Error| {
        trf!(
            "Jam impossible à démarrer : {}",
            "could not start the Jam: {}",
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

/// Tient `JamInfo` à jour d'après la connexion au relais et prévient l'UI.
fn relay_notify(
    app: AppHandle,
    info: Arc<Mutex<JamInfo>>,
    guests: Arc<Mutex<Guests>>,
) -> relay::Notify {
    Arc::new(move |event| {
        let changed = match info.lock() {
            Ok(mut info) => apply(&mut info, event),
            Err(_) => false,
        };
        if changed
            && let Some(status) = status_of(&info, &guests)
            && let Err(e) = app.emit(STATUS_EVENT, status)
        {
            tracing::warn!("Jam : {e}");
        }
    })
}

/// Applique un événement du relais ; vrai si l'UI doit être prévenue.
fn apply(info: &mut JamInfo, event: RelayEvent) -> bool {
    let before = info.clone();
    match event {
        RelayEvent::Connecting => {
            if info.url.is_none() {
                info.connection = Connection::Connecting;
            }
        }
        RelayEvent::Ready { url, .. } => {
            if info.url.as_deref() != Some(&url) {
                info.qr_svg = qr_svg(&format!("{url}#{}", info.code));
                info.url = Some(url);
            }
            info.connection = Connection::Online;
            info.reason = None;
        }
        RelayEvent::Lost { reason } => {
            info.connection = if info.url.is_some() {
                Connection::Reconnecting
            } else {
                Connection::Connecting
            };
            info.reason = reason;
        }
        RelayEvent::Failed { reason } => {
            info.connection = Connection::Failed;
            info.reason = Some(reason);
        }
    }
    *info != before
}

/// SVG sans la déclaration XML (inséré tel quel dans la page Réglages).
fn qr_svg(text: &str) -> Option<String> {
    let code = QrCode::new(text.as_bytes()).ok()?;
    let svg = code
        .render::<svg::Color>()
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .quiet_zone(true)
        .build();
    svg.find("<svg").map(|i| svg[i..].to_owned())
}

/// Adresse de cette machine sur le réseau local : celle de l'interface de la route par
/// défaut. `connect` sur UDP ne fait que choisir la route, rien n'est envoyé.
fn lan_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

struct AppBackend {
    app: AppHandle,
    tidal: Arc<TidalClient>,
    player: mpsc::Sender<Cmd>,
}

fn stopped() -> String {
    tr("lecteur arrêté", "player stopped").to_owned()
}

impl Backend for AppBackend {
    async fn search(&self, query: String) -> Result<Vec<Track>, String> {
        self.tidal
            .search(&query, SEARCH_LIMIT)
            .await
            .map(|r| r.tracks)
            .map_err(|e| e.to_string())
    }

    async fn enqueue(&self, id: TrackId) -> Result<(), String> {
        self.player
            .send(Cmd::AddToQueue(id))
            .await
            .map_err(|_| stopped())
    }

    async fn state(&self) -> Result<Snapshot, String> {
        let (tx, rx) = oneshot::channel();
        self.player
            .send(Cmd::GetState(tx))
            .await
            .map_err(|_| stopped())?;
        let player = rx.await.map_err(|_| stopped())?;
        let (tx, rx) = oneshot::channel();
        self.player
            .send(Cmd::GetQueue(tx))
            .await
            .map_err(|_| stopped())?;
        let queue = rx.await.map_err(|_| stopped())?;
        Ok(Snapshot {
            now: player.track,
            paused: player.paused,
            manual: queue.manual,
            upcoming: queue.upcoming,
        })
    }

    fn guests_changed(&self, guests: &Guests) {
        if let Err(e) = self.app.emit(GUESTS_EVENT, guests.snapshot()) {
            tracing::warn!("Jam : {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_code_is_a_bare_svg() {
        let svg = qr_svg("http://192.168.1.20:47800/#AB23CD").unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("#ffffff"));
        assert!(!svg.contains("<?xml"));
    }

    fn online() -> JamInfo {
        JamInfo {
            mode: JamMode::Online,
            url: None,
            code: "AB23CD".into(),
            lan: true,
            qr_svg: None,
            connection: Connection::Connecting,
            reason: None,
        }
    }

    #[test]
    fn relay_events_drive_the_status() {
        let mut info = online();
        assert!(!apply(&mut info, RelayEvent::Connecting));
        let url = "https://jam.example/j/abc/".to_owned();
        assert!(apply(
            &mut info,
            RelayEvent::Ready {
                url: url.clone(),
                resumed: false
            }
        ));
        assert_eq!(info.connection, Connection::Online);
        assert!(info.qr_svg.is_some());

        // Coupure : la même adresse reste affichée pendant la reconnexion.
        assert!(apply(&mut info, RelayEvent::Lost { reason: None }));
        assert_eq!(info.connection, Connection::Reconnecting);
        assert!(!apply(&mut info, RelayEvent::Connecting));
        assert_eq!(info.url.as_deref(), Some(url.as_str()));

        // Reprise impossible : nouvelle adresse, nouveau QR code.
        let qr = info.qr_svg.clone();
        assert!(apply(
            &mut info,
            RelayEvent::Ready {
                url: "https://jam.example/j/def/".into(),
                resumed: false
            }
        ));
        assert_ne!(info.qr_svg, qr);

        assert!(apply(
            &mut info,
            RelayEvent::Failed {
                reason: "clé".into()
            }
        ));
        assert_eq!(info.connection, Connection::Failed);
    }
}
