//! Télécommande (#336) : piloter le lecteur du PC depuis le navigateur d'un téléphone, sur
//! le réseau local.
//!
//! Réutilise le serveur de la Jam (`server.rs`, HTTP, flux d'état, garde-fous) sur un port
//! à part (47801, sinon un port libre) : elle tourne sans Jam, et le serveur de la Jam ne
//! connaît aucune de ses commandes. Le code de propriétaire (12 caractères) est gardé
//! d'un lancement à l'autre (`commands::remote`) : le QR code ne se scanne qu'une fois.
//! Bureau seulement, jamais par le relais en ligne.

mod api;
mod view;

use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tokio::net::TcpListener;
use tokio::sync::{Notify, mpsc, oneshot};
use tokio::task::JoinHandle;

use super::http::Response;
use super::server::{STYLE, Site, serve_site};
use super::{lan_ip, qr_svg};
use crate::content::filtered;
use crate::i18n::{tr, trf};
use crate::player::{Cmd, PlayerState, QueueView};
use crate::tidal::TidalClient;
use crate::tidal::models::{PlaylistSummary, SearchResults, Track};
pub use api::OWNER_CODE_LEN;
use api::{Command, Owner, Remote};

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
    pub url: String,
    pub code: String,
    /// Faux si aucune adresse du réseau local n'a été trouvée (`127.0.0.1`).
    pub lan: bool,
    /// QR code de `url#code` en SVG (noir sur blanc).
    pub qr_svg: Option<String>,
}

struct Running {
    status: RemoteStatus,
    owner: Arc<Owner>,
    task: JoinHandle<()>,
}

#[derive(Default)]
pub struct RemoteState(Mutex<Option<Running>>);

impl RemoteState {
    pub fn status(&self) -> Option<RemoteStatus> {
        let slot = self.0.lock().ok()?;
        slot.as_ref().map(|r| r.status.clone())
    }

    /// Démarre la télécommande avec `code`, ou renvoie celle qui tourne déjà.
    pub async fn start(
        &self,
        code: String,
        tidal: Arc<TidalClient>,
        player: mpsc::Sender<Cmd>,
    ) -> Result<RemoteStatus, String> {
        if cfg!(mobile) {
            return Err(tr(
                "la télécommande n'existe que sur l'ordinateur",
                "the remote is only available on the computer",
            )
            .to_owned());
        }
        if let Some(status) = self.status() {
            return Ok(status);
        }
        let (listener, url, lan) = listen().await?;
        let owner = Arc::new(Owner::new(code.clone()));
        let site = Arc::new(RemoteSite {
            owner: Arc::clone(&owner),
            remote: AppRemote { tidal, player },
        });
        let task = tokio::spawn(serve_site(listener, site));
        let status = RemoteStatus {
            qr_svg: qr_svg(&format!("{url}#{code}")),
            url,
            code,
            lan,
        };
        let mut slot = self
            .0
            .lock()
            .map_err(|_| tr("télécommande indisponible", "remote unavailable").to_owned())?;
        // Deux démarrages simultanés : le premier arrivé reste.
        if let Some(running) = slot.as_ref() {
            task.abort();
            return Ok(running.status.clone());
        }
        tracing::info!("Télécommande démarrée");
        *slot = Some(Running {
            status: status.clone(),
            owner,
            task,
        });
        Ok(status)
    }

    /// Remplace le code de la télécommande en cours : les téléphones déjà appairés devront
    /// scanner le nouveau QR code.
    pub fn set_code(&self, code: String) -> Option<RemoteStatus> {
        let mut slot = self.0.lock().ok()?;
        let running = slot.as_mut()?;
        running.owner.set_code(code.clone());
        running.status.qr_svg = qr_svg(&format!("{}#{code}", running.status.url));
        running.status.code = code;
        Some(running.status.clone())
    }

    /// Arrête la télécommande : port fermé, pages coupées.
    pub fn stop(&self) {
        if let Some(running) = self.0.lock().ok().and_then(|mut s| s.take()) {
            running.task.abort();
            tracing::info!("Télécommande arrêtée");
        }
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
}

struct AppRemote {
    tidal: Arc<TidalClient>,
    player: mpsc::Sender<Cmd>,
}

fn stopped() -> String {
    tr("lecteur arrêté", "player stopped").to_owned()
}

impl AppRemote {
    async fn send(&self, cmd: Cmd) -> Result<(), String> {
        self.player.send(cmd).await.map_err(|_| stopped())
    }
}

/// Traduit une commande de la télécommande pour l'acteur du lecteur.
fn to_cmd(command: Command) -> Cmd {
    match command {
        Command::SetPaused(paused) => Cmd::SetPaused(paused),
        Command::Next => Cmd::Next,
        Command::Previous => Cmd::Previous,
        Command::Seek(seconds) => Cmd::Seek(seconds),
        Command::Volume(level) => Cmd::SetVolume(level),
        Command::Muted(muted) => Cmd::SetMuted(muted),
        Command::Shuffle(enabled) => Cmd::SetShuffle(enabled),
        Command::Repeat(mode) => Cmd::SetRepeat(mode),
        Command::Jump { part, index, id } => Cmd::PlayFromQueue { part, index, id },
        Command::Remove(id) => Cmd::RemoveFromQueueIds(vec![id]),
        Command::Play {
            ids,
            shuffle,
            source,
        } => Cmd::PlayTracks {
            ids,
            start: 0,
            shuffle,
            source: source.and_then(|s| s.sanitize(crate::commands::settings::pins::is_uuid)),
        },
        Command::Enqueue { ids, next } => Cmd::Enqueue { ids, next },
    }
}

impl Remote for AppRemote {
    async fn state(&self) -> Result<(PlayerState, QueueView), String> {
        let (tx, rx) = oneshot::channel();
        self.send(Cmd::GetState(tx)).await?;
        let player = rx.await.map_err(|_| stopped())?;
        let (tx, rx) = oneshot::channel();
        self.send(Cmd::GetQueue(tx)).await?;
        let queue = rx.await.map_err(|_| stopped())?;
        Ok((player, queue))
    }

    async fn run(&self, command: Command) -> Result<(), String> {
        self.send(to_cmd(command)).await
    }

    async fn search(&self, query: String) -> Result<SearchResults, String> {
        self.tidal
            .search(&query, SEARCH_LIMIT)
            .await
            .map(filtered)
            .map_err(|e| e.to_string())
    }

    async fn album(&self, id: u64) -> Result<(String, Vec<Track>), String> {
        let page = self.tidal.album(id).await.map_err(|e| e.to_string())?;
        let page = filtered(page);
        Ok((page.album.title, page.tracks))
    }

    async fn playlist(&self, uuid: String) -> Result<(String, Vec<Track>), String> {
        let page = self
            .tidal
            .playlist(&uuid)
            .await
            .map_err(|e| e.to_string())?;
        let page = filtered(page);
        let tracks = page.tracks.into_iter().map(|e| e.track).collect();
        Ok((page.playlist.title, tracks))
    }

    async fn playlists(&self) -> Result<Vec<PlaylistSummary>, String> {
        self.tidal
            .library_playlists()
            .await
            .map(|list| list.items)
            .map_err(|e| e.to_string())
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
