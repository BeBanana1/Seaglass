//! Relais de la Jam Seaglass (#25, étape 3a) : rend une Jam joignable depuis Internet,
//! sans Wi-Fi commun ni port ouvert chez l'hôte.
//!
//! L'app de l'hôte se connecte en WebSocket (`/host`) ; les invités ouvrent
//! `/j/<id>/` (page servie ici, la même que la Jam locale) et leurs appels de l'API sont
//! transmis à l'hôte, qui seul parle à TIDAL et au lecteur. Le relais ne voit passer que
//! des titres, des prénoms et le code de la Jam ; jamais de jeton TIDAL ni de flux audio.
//!
//! Il relie aussi un PC à son téléphone lié (#430, `/device` et `/d/<id>`) : transfert de
//! la lecture et historique d'écoute, chiffrés de bout en bout, illisibles ici.

mod device;
pub mod devices;
mod guest;
mod host;
pub mod jams;
pub mod limits;
pub mod protocol;

use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::HeaderMap;
use axum::routing::{get, post};
use tokio::net::TcpListener;

use devices::Devices;
use jams::Registry;
use limits::Rate;

/// Corps d'une requête d'invité (comme la Jam locale).
const MAX_BODY: usize = 2 * 1024;
/// Message de l'hôte (état de 20 titres, réponse de 30 résultats).
const MAX_HOST_MESSAGE: usize = 256 * 1024;
/// Paquet chiffré du téléphone vers son PC (lot d'historique, file transférée : 4 Mio au
/// plus en clair, base64 compris).
const MAX_DEVICE_BODY: usize = 8 * 1024 * 1024;
/// Message d'un PC (réponse chiffrée, enveloppe JSON comprise).
const MAX_DEVICE_MESSAGE: usize = 12 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Config {
    /// Adresse publique sans `/` final, par exemple `https://jam.bebanana.ch`.
    pub public_url: String,
    /// Clé de build de l'app (`SEAGLASS_RELAY_KEY`) : écarte les robots, pas un secret.
    pub host_key: String,
    pub max_jams: usize,
    /// Jams ouvertes en même temps pour une même adresse d'hôte (#257) : la clé de build
    /// se lit dans l'app, une seule adresse ne doit pas pouvoir remplir le relais.
    pub max_jams_per_address: usize,
    /// Durée de vie d'une Jam, reprises comprises.
    pub lifetime: Duration,
    /// Délai laissé à l'hôte pour revenir après une coupure (réseau mobile).
    pub grace: Duration,
    /// Adresse du client lue dans `X-Forwarded-For` (derrière Caddy seulement).
    pub trust_proxy: bool,
    /// Jams créées par adresse et par heure.
    pub creations_per_hour: usize,
    /// Appels de l'API par adresse et par minute (toutes Jams confondues). Large : en données
    /// mobiles, plusieurs invités sortent par la même adresse et cherchent pendant la saisie
    /// (#256) ; l'hôte compte ensuite par invité.
    pub requests_per_minute: usize,
    /// Pages ouvertes par Jam.
    pub max_streams: usize,
    /// PC connectés en même temps (#430).
    pub max_devices: usize,
    /// Dont ceux d'une même adresse.
    pub max_devices_per_address: usize,
    /// Appels des téléphones par adresse et par minute.
    pub device_calls_per_minute: usize,
}

impl Config {
    pub fn new(public_url: &str, host_key: &str) -> Self {
        Self {
            public_url: public_url.trim_end_matches('/').to_owned(),
            host_key: host_key.to_owned(),
            max_jams: 20,
            max_jams_per_address: 3,
            lifetime: Duration::from_secs(12 * 3600),
            grace: Duration::from_secs(30 * 60),
            trust_proxy: false,
            creations_per_hour: 10,
            requests_per_minute: 300,
            max_streams: 64,
            max_devices: 200,
            max_devices_per_address: 3,
            device_calls_per_minute: 120,
        }
    }
}

pub(crate) struct Relay {
    config: Config,
    jams: Registry,
    devices: Devices,
    creations: Mutex<Rate>,
    requests: Mutex<Rate>,
    device_calls: Mutex<Rate>,
}

impl Relay {
    fn client_ip(&self, peer: SocketAddr, headers: &HeaderMap) -> IpAddr {
        if self.config.trust_proxy
            && let Some(ip) = headers
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.rsplit(',').next())
                .and_then(|v| v.trim().parse().ok())
        {
            return ip;
        }
        match peer.ip() {
            IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(IpAddr::V6(v6), IpAddr::V4),
            ip => ip,
        }
    }

    fn allow(rate: &Mutex<Rate>, ip: IpAddr) -> bool {
        rate.lock()
            .unwrap_or_else(|e| e.into_inner())
            .allow(ip, Instant::now())
    }
}

/// Sert le relais sur `listener` jusqu'à l'arrêt du processus.
pub async fn serve(listener: TcpListener, config: Config) -> std::io::Result<()> {
    let relay = Arc::new(Relay {
        creations: Mutex::new(Rate::new(
            config.creations_per_hour,
            Duration::from_secs(3600),
        )),
        requests: Mutex::new(Rate::new(
            config.requests_per_minute,
            Duration::from_secs(60),
        )),
        device_calls: Mutex::new(Rate::new(
            config.device_calls_per_minute,
            Duration::from_secs(60),
        )),
        jams: Registry::default(),
        devices: Devices::default(),
        config,
    });
    tokio::spawn(sweeper(Arc::clone(&relay)));
    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/host", get(host::connect))
        .route("/j/{id}", get(guest::redirect))
        .route("/j/{id}/", get(guest::page))
        .route("/j/{id}/jam.js", get(guest::script))
        .route("/j/{id}/jam.css", get(guest::style))
        .route("/j/{id}/api/events", get(guest::events))
        .route("/j/{id}/api/{call}", post(guest::api))
        .route("/device", get(device::connect))
        .route(
            "/d/{id}",
            post(device::call).layer(DefaultBodyLimit::max(MAX_DEVICE_BODY)),
        )
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .with_state(relay);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
}

/// Ferme les Jams expirées et oublie les adresses inactives.
async fn sweeper(relay: Arc<Relay>) {
    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;
        let now = Instant::now();
        relay
            .jams
            .sweep(now, relay.config.lifetime, relay.config.grace);
        for rate in [&relay.creations, &relay.requests, &relay.device_calls] {
            rate.lock().unwrap_or_else(|e| e.into_inner()).prune(now);
        }
    }
}
