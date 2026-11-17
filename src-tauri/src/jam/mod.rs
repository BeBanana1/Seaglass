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
mod lan;
mod members;
mod protocol;
mod relay;
pub mod remote;
mod rules;
mod server;
#[cfg(test)]
mod testing;
mod view;

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex};

use crate::content::filtered;
use qrcode::QrCode;
use qrcode::render::svg;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use lan::lan_ip;

use crate::i18n::{tr, trf};
use crate::player::Cmd;
use crate::player::blind::{Answer, BlindCmd, Refusal};
use crate::tidal::TidalClient;
use crate::tidal::models::{Track, TrackId};
use api::{Backend, Core, Hits, Snapshot};
use guests::Guests;
use members::GuestInfo;
use relay::{RelayConfig, RelayEvent};
pub use rules::JamRules;

/// Port préféré : l'adresse reste la même d'une Jam à l'autre (marque-page des invités).
const PORT: u16 = 47800;
const SEARCH_LIMIT: u32 = 30;
/// Événement vers l'UI de l'hôte : `GuestsUpdate` (noms des titres, invités).
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
    /// Règles de l'hôte (#222, #288), retenues d'une Jam à l'autre.
    pub rules: JamRules,
    /// Les invités peuvent ajouter (suspension propre à la Jam, #288).
    pub adds_open: bool,
    /// Invités qui ont ajouté, exclus compris (#288).
    pub members: Vec<GuestInfo>,
}

/// Charge de l'événement `jam-guests`.
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
        status_of(&running.info, &running.core)
    }

    /// Règles de l'hôte (#222, #288), appliquées à la Jam en cours s'il y en a une.
    pub fn set_rules(&self, rules: JamRules) -> Option<JamStatus> {
        let slot = self.0.lock().ok()?;
        let running = slot.as_ref()?;
        running.core.set_rules(rules);
        status_of(&running.info, &running.core)
    }

    /// Suspend ou rouvre les ajouts des invités (#288).
    pub fn set_adds_open(&self, open: bool) -> Option<JamStatus> {
        let slot = self.0.lock().ok()?;
        let running = slot.as_ref()?;
        running.core.set_adds_open(open);
        status_of(&running.info, &running.core)
    }

    /// Exclut un invité (ses titres en attente sont retirés) ou le réadmet (#288).
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

    /// Démarre la Jam, ou renvoie celle qui tourne déjà.
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
        let code = guard::new_code();
        let guests: Arc<Mutex<Guests>> = Arc::default();
        // Réseau local : une adresse = un appareil, l'exclusion la bloque aussi (#288).
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
        // Deux démarrages simultanés : le premier arrivé reste.
        if slot.is_some() {
            task.abort();
            drop(slot);
            return self
                .status()
                .ok_or_else(|| tr("Jam indisponible", "Jam unavailable").to_owned());
        }
        tracing::info!("Jam démarrée ({mode:?})");
        let status = status_of(&info, &core);
        *slot = Some(Running {
            info,
            core,
            backend,
            task,
        });
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

struct AppBackend {
    app: AppHandle,
    tidal: Arc<TidalClient>,
    player: mpsc::Sender<Cmd>,
}

fn stopped() -> String {
    tr("lecteur arrêté", "player stopped").to_owned()
}

impl Backend for AppBackend {
    async fn search(&self, query: String) -> Result<Hits, String> {
        self.tidal
            .search(&query, SEARCH_LIMIT)
            .await
            .map(filtered)
            .map(|r| Hits {
                tracks: r.tracks,
                albums: r.albums,
            })
            .map_err(|e| e.to_string())
    }

    async fn link(&self, text: String) -> Result<Hits, String> {
        match crate::foreign::resolve(&self.tidal, &text).await {
            Some(Ok(found)) => Ok(Hits {
                tracks: filtered(found.tracks),
                albums: filtered(found.albums),
            }),
            Some(Err(e)) => Err(trf!("lien illisible : {}", "unreadable link: {}", e)),
            None => Ok(Hits::default()),
        }
    }

    async fn album_tracks(&self, id: u64) -> Result<Vec<Track>, String> {
        self.tidal
            .album(id)
            .await
            .map(|page| filtered(page.tracks))
            .map_err(|e| e.to_string())
    }

    async fn enqueue(&self, ids: Vec<TrackId>) -> Result<(), String> {
        self.player
            .send(Cmd::Enqueue { ids, next: false })
            .await
            .map_err(|_| stopped())
    }

    async fn remove(&self, ids: Vec<TrackId>) -> Result<(), String> {
        self.player
            .send(Cmd::RemoveFromQueueIds(ids))
            .await
            .map_err(|_| stopped())
    }

    async fn set_paused(&self, paused: bool) -> Result<(), String> {
        self.player
            .send(Cmd::SetPaused(paused))
            .await
            .map_err(|_| stopped())
    }

    async fn set_volume(&self, level: u8) -> Result<(), String> {
        self.player
            .send(Cmd::SetVolume(level))
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
        // La partie n'est demandée que si l'état en annonce une (#441).
        let blind = if player.blind_test {
            let (tx, rx) = oneshot::channel();
            self.player
                .send(Cmd::BlindTest(BlindCmd::Get(tx)))
                .await
                .map_err(|_| stopped())?;
            rx.await.map_err(|_| stopped())?
        } else {
            None
        };
        Ok(Snapshot {
            now: player.track,
            paused: player.paused,
            manual: queue.manual,
            upcoming: queue.upcoming,
            blind,
            volume: player.volume,
        })
    }

    async fn blind_answer(
        &self,
        key: String,
        name: Option<String>,
        round: usize,
        answer: Answer,
    ) -> Result<Result<u32, Refusal>, String> {
        let (reply, rx) = oneshot::channel();
        let cmd = BlindCmd::GuestAnswer {
            key,
            name,
            round,
            answer,
            reply,
        };
        self.player
            .send(Cmd::BlindTest(cmd))
            .await
            .map_err(|_| stopped())?;
        rx.await.map_err(|_| stopped())
    }

    fn guests_changed(&self, guests: &Guests) {
        let update = GuestsUpdate {
            guests: guests.snapshot(),
            members: guests.members.list(),
        };
        if let Err(e) = self.app.emit(GUESTS_EVENT, update) {
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
