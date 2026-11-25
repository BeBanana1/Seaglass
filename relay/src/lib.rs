//! Seaglass Jam relay (#25, step 3a): makes a Jam reachable from the Internet, with no
//! shared Wi-Fi and no open port at the host's.
//!
//! The host's app connects over WebSocket (`/host`); guests open `/j/<id>/` (page served
//! here, the same as the local Jam) and their API calls are forwarded to the host, the only
//! one talking to TIDAL and the player. The relay only sees titles, first names and the
//! Jam code; never a TIDAL token nor an audio stream.
//!
//! It also links a PC to its paired phone (#430, `/device` and `/d/<id>`): playback handoff
//! and listening history, end-to-end encrypted, unreadable here.
//!
//! **Request bodies** are read by the handlers themselves (`read_body`), only once the
//! rate limit and the target (Jam, device) have been checked: an `axum` body extractor
//! would buffer the whole body before any check, and the container only has 128 MiB.

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
use axum::body::{Body, Bytes};
use axum::extract::DefaultBodyLimit;
use axum::http::HeaderMap;
use axum::routing::{get, post};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

use devices::Devices;
use jams::Registry;
use limits::Rate;

/// Body of a guest request (like the local Jam).
const MAX_BODY: usize = 2 * 1024;
/// Message from the host (state of 20 tracks, answer with 30 results).
const MAX_HOST_MESSAGE: usize = 256 * 1024;
/// Encrypted packet from the phone to its PC (history batch, handed-off queue: 4 MiB at
/// most in plain text, base64 included).
const MAX_DEVICE_BODY: usize = 8 * 1024 * 1024;
/// Message from a PC (encrypted answer, JSON envelope included).
const MAX_DEVICE_MESSAGE: usize = 12 * 1024 * 1024;
/// Phone packets held in memory at the same time, all devices together, in KiB. A packet
/// is held twice while forwarded (body, then the message to the PC): 2 × 32 MiB stays well
/// under the container's 128 MiB. A packet reserves its `Content-Length`, the maximum
/// without one; beyond the budget, `503 busy` (the phone already retries on it).
const DEVICE_UPLOAD_BUDGET_KIB: usize = 32 * 1024;

#[derive(Debug, Clone)]
pub struct Config {
    /// Public address without a trailing `/`, for example `https://jam.example.org`.
    pub public_url: String,
    /// Build key of the app (`SEAGLASS_RELAY_KEY`): keeps robots out, not a secret.
    pub host_key: String,
    pub max_jams: usize,
    /// Jams open at the same time for one host address (#257): the build key can be read
    /// in the app, a single address must not be able to fill the relay.
    pub max_jams_per_address: usize,
    /// Lifetime of a Jam, resumes included.
    pub lifetime: Duration,
    /// Time left to the host to come back after a cut (mobile network).
    pub grace: Duration,
    /// Client address read from `X-Forwarded-For` (behind Caddy only).
    pub trust_proxy: bool,
    /// Jams created per address and per hour.
    pub creations_per_hour: usize,
    /// API calls per address and per minute (all Jams together). Generous: on mobile data,
    /// several guests go out through the same address and search while typing (#256); the
    /// host then counts per guest.
    pub requests_per_minute: usize,
    /// Pages open per Jam.
    pub max_streams: usize,
    /// PCs connected at the same time (#430).
    pub max_devices: usize,
    /// Of which from the same address.
    pub max_devices_per_address: usize,
    /// Phone calls per address and per minute.
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
    /// Memory left for phone packets, in KiB (`DEVICE_UPLOAD_BUDGET_KIB`).
    device_uploads: Semaphore,
}

impl Relay {
    /// Address counted by every limit and passed to the host: see `limits::network`.
    fn client_ip(&self, peer: SocketAddr, headers: &HeaderMap) -> IpAddr {
        let forwarded = self
            .config
            .trust_proxy
            .then(|| {
                headers
                    .get("x-forwarded-for")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.rsplit(',').next())
                    .and_then(|v| v.trim().parse().ok())
            })
            .flatten();
        limits::network(forwarded.unwrap_or(peer.ip()))
    }

    fn allow(rate: &Mutex<Rate>, ip: IpAddr) -> bool {
        rate.lock()
            .unwrap_or_else(|e| e.into_inner())
            .allow(ip, Instant::now())
    }
}

/// Declared `Content-Length`, `None` if absent or unreadable.
fn content_length(headers: &HeaderMap) -> Option<usize> {
    headers
        .get(axum::http::header::CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .parse()
        .ok()
}

/// Reads a body of at most `max` bytes (and at most its declared `Content-Length`): `None`
/// if it is larger or cut. To call only once the request has passed its checks.
async fn read_body(body: Body, headers: &HeaderMap, max: usize) -> Option<Bytes> {
    let limit = match content_length(headers) {
        Some(declared) if declared > max => return None,
        Some(declared) => declared,
        None => max,
    };
    axum::body::to_bytes(body, limit).await.ok()
}

/// Bytes into text without a copy when they are not shared.
fn into_text(bytes: Bytes) -> Option<String> {
    String::from_utf8(Vec::from(bytes)).ok()
}

/// Serves the relay on `listener` until the process stops.
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
        device_uploads: Semaphore::new(DEVICE_UPLOAD_BUDGET_KIB),
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
        .route("/d/{id}", post(device::call))
        // Bodies are read by `read_body` with their own limit: this one only covers a
        // future route that would use an `axum` body extractor.
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .with_state(relay);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
}

/// Closes the expired Jams and forgets the inactive addresses.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(length: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(length) = length {
            headers.insert("content-length", length.parse().unwrap());
        }
        headers
    }

    async fn read(body: &'static str, length: Option<&str>, max: usize) -> Option<Bytes> {
        read_body(Body::from(body), &headers(length), max).await
    }

    #[tokio::test]
    async fn bodies_are_read_within_their_limits() {
        assert_eq!(
            read("abc", Some("3"), 8).await.as_deref(),
            Some(&b"abc"[..])
        );
        assert_eq!(read("abc", None, 8).await.as_deref(), Some(&b"abc"[..]));
        // Declared too large: refused without reading.
        assert!(read("abc", Some("9"), 8).await.is_none());
        // Larger than declared, or than the limit without a declaration.
        assert!(read("abcdef", Some("3"), 8).await.is_none());
        assert!(read("abcdefghij", None, 8).await.is_none());
    }

    #[test]
    fn text_is_utf8_only() {
        assert_eq!(into_text(Bytes::from_static(b"ok")).as_deref(), Some("ok"));
        assert!(into_text(Bytes::from_static(b"\xff")).is_none());
    }
}
