//! Jam locale (#25) : des invités sur le même réseau ajoutent des titres à la file de
//! l'hôte depuis une page web, sans app ni compte. Seule la machine de l'hôte lit.
//!
//! Démarrer ouvre un port sur le réseau local (47800, sinon un port libre) avec un code
//! d'accès tiré pour cette Jam ; arrêter ferme le port et rend le code invalide. Le QR code
//! porte l'adresse et le code (après `#`, jamais envoyé au serveur ni journalisé).

mod guard;
mod guests;
mod http;
mod server;

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::sync::{Arc, Mutex};

use qrcode::QrCode;
use qrcode::render::svg;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::i18n::{tr, trf};
use crate::player::Cmd;
use crate::tidal::TidalClient;
use crate::tidal::models::{Track, TrackId};
use guests::Guests;
use server::{Backend, Snapshot};

/// Port préféré : l'adresse reste la même d'une Jam à l'autre (marque-page des invités).
const PORT: u16 = 47800;
const SEARCH_LIMIT: u32 = 30;
/// Événement vers l'UI de l'hôte : `{ <id du titre>: <nom de l'invité> }`.
const GUESTS_EVENT: &str = "jam-guests";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JamInfo {
    /// `http://<adresse locale>:<port>/`.
    pub url: String,
    pub code: String,
    /// Faux si aucune adresse du réseau local n'a été trouvée (adresse `127.0.0.1`).
    pub lan: bool,
    /// QR code de `url#code` en SVG (noir sur blanc), `None` si l'encodage a échoué.
    pub qr_svg: Option<String>,
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
    info: JamInfo,
    guests: Arc<Mutex<Guests>>,
    task: JoinHandle<()>,
}

#[derive(Default)]
pub struct JamState(Mutex<Option<Running>>);

impl JamState {
    pub fn status(&self) -> Option<JamStatus> {
        let slot = self.0.lock().ok()?;
        let running = slot.as_ref()?;
        Some(JamStatus {
            info: running.info.clone(),
            guests: running
                .guests
                .lock()
                .map(|g| g.snapshot())
                .unwrap_or_default(),
        })
    }

    /// Démarre la Jam, ou renvoie celle qui tourne déjà.
    pub async fn start(
        &self,
        app: AppHandle,
        tidal: Arc<TidalClient>,
        player: mpsc::Sender<Cmd>,
    ) -> Result<JamStatus, String> {
        if let Some(status) = self.status() {
            return Ok(status);
        }
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
        let code = guard::new_code();
        let info = JamInfo {
            qr_svg: qr_svg(&format!("{url}#{code}")),
            url,
            code,
            lan: ip.is_some(),
        };
        let guests: Arc<Mutex<Guests>> = Arc::default();
        let backend = Arc::new(AppBackend { app, tidal, player });
        let task = tokio::spawn(server::serve(
            listener,
            info.code.clone(),
            Arc::clone(&guests),
            backend,
        ));
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
        tracing::info!("Jam démarrée sur le port {port}");
        *slot = Some(Running {
            info: info.clone(),
            guests,
            task,
        });
        Ok(JamStatus {
            info,
            guests: HashMap::new(),
        })
    }

    /// Arrête la Jam : le port est fermé, les flux ouverts et les connexions en cours se
    /// terminent seuls.
    pub fn stop(&self) {
        if let Some(running) = self.0.lock().ok().and_then(|mut s| s.take()) {
            running.task.abort();
            tracing::info!("Jam arrêtée");
        }
    }
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
}
