//! Jam (#25): guests add tracks to the host's queue from a web page, with no app or
//! account. Only the host's machine plays.
//!
//! Two modes, same rules (`api.rs`), and the app's side of them in `backend.rs`:
//! - **Online** (`relay.rs`): the app connects to the relay (`relay/`), guests open
//!   `https://<relay>/j/<id>/`, with no shared Wi-Fi. Relay built in or chosen in Settings
//!   (#452, `relay_settings.rs`).
//! - **Local network** (`server.rs`): the app opens a port (47800, otherwise a free one).
//!   Desktop only: on a phone on mobile data (or sharing its connection), the address would
//!   be useless, and the app opens no port on Android.
//!
//! The access code is drawn for each Jam; stopping makes the code invalid. The QR code
//! carries the address and the code (after `#`, never sent to the server or logged).

mod api;
mod backend;
mod clock;
mod guard;
mod guests;
mod http;
mod lan;
mod members;
mod protocol;
mod relay;
pub mod relay_settings;
pub mod remote;
mod rules;
mod server;
#[cfg(test)]
mod testing;
mod view;

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use qrcode::QrCode;
use qrcode::render::svg;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use lan::lan_ip;

use crate::i18n::{tr, trf};
use crate::player::Cmd;
use crate::tidal::TidalClient;
use api::Core;
use backend::AppBackend;
use guests::Guests;
use members::GuestInfo;
use relay::{RelayConfig, RelayEvent};
pub use rules::JamRules;

/// Preferred port: the address stays the same from one Jam to the next (guests' bookmark).
const PORT: u16 = 47800;
const SEARCH_LIMIT: u32 = 30;
/// Event to the host's UI: `GuestsUpdate` (names on the tracks, guests).
const GUESTS_EVENT: &str = "jam-guests";
/// Event to the host's UI: `JamStatus` (relay connection, address changed).
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
    /// Final refusal by the relay (key): the Jam stays shown until "Stop".
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JamInfo {
    pub mode: JamMode,
    /// Address to give the guests; `None` until the relay has answered.
    pub url: Option<String>,
    pub code: String,
    /// Local network: false if no network address was found (`127.0.0.1`).
    pub lan: bool,
    /// QR code of `url#code` as SVG (black on white), `None` without an address.
    pub qr_svg: Option<String>,
    pub connection: Connection,
    /// Reason for a reconnection or a failure, translated.
    pub reason: Option<String>,
}

/// State of the Jam for the host's UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JamStatus {
    #[serde(flatten)]
    pub info: JamInfo,
    /// Name of the guest on each track the Jam added.
    pub guests: HashMap<u64, String>,
    /// The host's rules (#222, #288), kept from one Jam to the next.
    pub rules: JamRules,
    /// Guests may add (suspension specific to the Jam, #288).
    pub adds_open: bool,
    /// Guests who added, banned ones included (#288).
    pub members: Vec<GuestInfo>,
}

/// Payload of the `jam-guests` event.
#[derive(Debug, Clone, Serialize)]
struct GuestsUpdate {
    guests: HashMap<u64, String>,
    members: Vec<GuestInfo>,
}

struct Running {
    info: Arc<Mutex<JamInfo>>,
    core: Arc<Core>,
    backend: Arc<AppBackend>,
    task: JoinHandle<()>,
}

fn status_of(info: &Mutex<JamInfo>, core: &Core) -> Option<JamStatus> {
    Some(JamStatus {
        info: info.lock().ok()?.clone(),
        guests: core.guest_names(),
        rules: core.rules(),
        adds_open: core.adds_open(),
        members: core.members(),
    })
}

/// "Jam" setting (#452): off, it disappears from the UI and no longer starts.
static ENABLED: AtomicBool = AtomicBool::new(true);

pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// True if a relay can be used (address and key): the online Jam is offered.
pub fn online_available() -> bool {
    RelayConfig::current().is_some()
}

#[derive(Default)]
pub struct JamState(Mutex<Option<Running>>);

impl JamState {
    pub fn status(&self) -> Option<JamStatus> {
        let slot = self.0.lock().ok()?;
        let running = slot.as_ref()?;
        status_of(&running.info, &running.core)
    }

    /// The host's rules (#222, #288), applied to the running Jam if there is one.
    pub fn set_rules(&self, rules: JamRules) -> Option<JamStatus> {
        let slot = self.0.lock().ok()?;
        let running = slot.as_ref()?;
        running.core.set_rules(rules);
        status_of(&running.info, &running.core)
    }

    /// Suspends or reopens the guests' additions (#288).
    pub fn set_adds_open(&self, open: bool) -> Option<JamStatus> {
        let slot = self.0.lock().ok()?;
        let running = slot.as_ref()?;
        running.core.set_adds_open(open);
        status_of(&running.info, &running.core)
    }

    /// Bans a guest (their pending tracks are removed) or lets them back in (#288).
    pub async fn set_guest_banned(&self, id: u32, banned: bool) -> Option<JamStatus> {
        let (info, core, backend) = {
            let slot = self.0.lock().ok()?;
            let running = slot.as_ref()?;
            (
                Arc::clone(&running.info),
                Arc::clone(&running.core),
                Arc::clone(&running.backend),
            )
        };
        core.set_banned(id, banned, backend.as_ref()).await;
        status_of(&info, &core)
    }

    /// Starts the Jam, or returns the one already running.
    pub async fn start(
        &self,
        mode: JamMode,
        rules: JamRules,
        app: AppHandle,
        tidal: Arc<TidalClient>,
        player: mpsc::Sender<Cmd>,
    ) -> Result<JamStatus, String> {
        if let Some(status) = self.status() {
            return Ok(status);
        }
        if !enabled() {
            return Err(tr(
                "la Jam est désactivée dans les Réglages",
                "the Jam is turned off in Settings",
            )
            .to_owned());
        }
        let code = guard::new_code();
        let guests: Arc<Mutex<Guests>> = Arc::default();
        // Local network: one address = one device, a ban blocks it too (#288).
        let ban_ips = mode == JamMode::Local;
        let core = Arc::new(Core::with_rules(
            code.clone(),
            Arc::clone(&guests),
            rules,
            ban_ips,
        ));
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
                let task = tokio::spawn(server::serve(
                    listener,
                    Arc::clone(&core),
                    Arc::clone(&backend),
                ));
                (info, task)
            }
            JamMode::Online => {
                let config = RelayConfig::current().ok_or_else(|| {
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
                let notify = relay_notify(app, Arc::clone(&info), Arc::clone(&core));
                let task = tokio::spawn(relay::run(
                    config,
                    code,
                    Arc::clone(&core),
                    Arc::clone(&backend),
                    notify,
                ));
                (info, task)
            }
        };
        let mut slot = self
            .0
            .lock()
            .map_err(|_| tr("Jam indisponible", "Jam unavailable").to_owned())?;
        // Two starts at the same time: the first one stays.
        if slot.is_some() {
            task.abort();
            drop(slot);
            return self
                .status()
                .ok_or_else(|| tr("Jam indisponible", "Jam unavailable").to_owned());
        }
        tracing::info!("Jam started ({mode:?})");
        let status = status_of(&info, &core);
        *slot = Some(Running {
            info,
            core,
            backend,
            task,
        });
        status.ok_or_else(|| tr("Jam indisponible", "Jam unavailable").to_owned())
    }

    /// Stops the Jam: port closed or relay connection cut, code invalid.
    pub fn stop(&self) {
        if let Some(running) = self.0.lock().ok().and_then(|mut s| s.take()) {
            running.task.abort();
            tracing::info!("Jam stopped");
        }
    }
}

/// Opens the local Jam's port: listener, address to give, local address found.
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

/// Keeps `JamInfo` up to date from the relay connection and tells the UI.
fn relay_notify(app: AppHandle, info: Arc<Mutex<JamInfo>>, core: Arc<Core>) -> relay::Notify {
    Arc::new(move |event| {
        let changed = match info.lock() {
            Ok(mut info) => apply(&mut info, event),
            Err(_) => false,
        };
        if changed
            && let Some(status) = status_of(&info, &core)
            && let Err(e) = app.emit(STATUS_EVENT, status)
        {
            tracing::warn!("Jam: {e}");
        }
    })
}

/// Applies a relay event; true if the UI must be told.
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

/// SVG without the XML declaration (inserted as is in the Settings page).
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

        // Disconnected: the same address stays shown during the reconnection.
        assert!(apply(&mut info, RelayEvent::Lost { reason: None }));
        assert_eq!(info.connection, Connection::Reconnecting);
        assert!(!apply(&mut info, RelayEvent::Connecting));
        assert_eq!(info.url.as_deref(), Some(url.as_str()));

        // Resume impossible: new address, new QR code.
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
