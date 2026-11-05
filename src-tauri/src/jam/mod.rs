//! Jam locale (#25) : des invités sur le même réseau ajoutent des titres à la file de
//! l'hôte depuis une page web, sans app ni compte. Seule la machine de l'hôte lit.
//!
//! Démarrer ouvre un port sur le réseau local (47800, sinon un port libre) avec un code
//! d'accès tiré pour cette Jam ; arrêter ferme le port et rend le code invalide.

mod guard;
mod http;
mod server;

use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::i18n::{tr, trf};
use crate::player::Cmd;
use crate::tidal::TidalClient;
use crate::tidal::models::{Track, TrackId};
use server::{Backend, GuestState};

/// Port préféré : l'adresse reste la même d'une Jam à l'autre (marque-page des invités).
const PORT: u16 = 47800;
const SEARCH_LIMIT: u32 = 30;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JamInfo {
    /// `http://<adresse locale>:<port>/`.
    pub url: String,
    pub code: String,
    /// Faux si aucune adresse du réseau local n'a été trouvée (adresse `127.0.0.1`).
    pub lan: bool,
}

struct Running {
    info: JamInfo,
    task: JoinHandle<()>,
}

#[derive(Default)]
pub struct JamState(Mutex<Option<Running>>);

impl JamState {
    pub fn info(&self) -> Option<JamInfo> {
        self.0.lock().ok()?.as_ref().map(|r| r.info.clone())
    }

    /// Démarre la Jam, ou renvoie celle qui tourne déjà.
    pub async fn start(
        &self,
        tidal: Arc<TidalClient>,
        player: mpsc::Sender<Cmd>,
    ) -> Result<JamInfo, String> {
        if let Some(info) = self.info() {
            return Ok(info);
        }
        let listener = match TcpListener::bind((Ipv4Addr::UNSPECIFIED, PORT)).await {
            Ok(l) => l,
            Err(_) => TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0))
                .await
                .map_err(|e| {
                    trf!(
                        "Jam impossible à démarrer : {}",
                        "could not start the Jam: {}",
                        e
                    )
                })?,
        };
        let port = listener
            .local_addr()
            .map_err(|e| {
                trf!(
                    "Jam impossible à démarrer : {}",
                    "could not start the Jam: {}",
                    e
                )
            })?
            .port();
        let ip = lan_ip();
        let info = JamInfo {
            url: format!(
                "http://{}:{port}/",
                ip.unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
            ),
            code: guard::new_code(),
            lan: ip.is_some(),
        };
        let backend = Arc::new(AppBackend { tidal, player });
        let task = tokio::spawn(server::serve(listener, info.code.clone(), backend));
        let mut slot = self
            .0
            .lock()
            .map_err(|_| tr("Jam indisponible", "Jam unavailable").to_owned())?;
        // Deux démarrages simultanés : le premier arrivé reste.
        if let Some(running) = slot.as_ref() {
            task.abort();
            return Ok(running.info.clone());
        }
        tracing::info!("Jam démarrée sur le port {port}");
        *slot = Some(Running {
            info: info.clone(),
            task,
        });
        Ok(info)
    }

    /// Arrête la Jam : le port est fermé, les connexions en cours se terminent seules.
    pub fn stop(&self) {
        if let Some(running) = self.0.lock().ok().and_then(|mut s| s.take()) {
            running.task.abort();
            tracing::info!("Jam arrêtée");
        }
    }
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

    async fn state(&self) -> Result<GuestState, String> {
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
        Ok(GuestState::new(
            player.track.as_ref(),
            player.paused,
            queue.manual.into_iter().chain(queue.upcoming),
        ))
    }
}
