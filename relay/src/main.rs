//! Seaglass Jam relay. Configuration through environment variables:
//!
//! - `SEAGLASS_RELAY_KEY` (required): the app's build key, the same as in its `.env`.
//! - `RELAY_PUBLIC_URL` (required): public address, for example `https://jam.example.org`.
//! - `RELAY_BIND`: listen address (default `0.0.0.0:8080`).
//! - `RELAY_TRUST_PROXY`: `1` behind Caddy (reads `X-Forwarded-For`), default `0`.
//! - `RELAY_MAX_JAMS`: Jams open at the same time (default 20).
//! - `RELAY_MAX_JAMS_PER_ADDRESS`: of which from the same host address (default 3).

use std::process::ExitCode;

use seaglass_relay::{Config, serve};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    let env = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    let Some(key) = env("SEAGLASS_RELAY_KEY") else {
        tracing::error!("SEAGLASS_RELAY_KEY missing");
        return ExitCode::FAILURE;
    };
    let Some(public_url) = env("RELAY_PUBLIC_URL") else {
        tracing::error!("RELAY_PUBLIC_URL missing (public address of the relay)");
        return ExitCode::FAILURE;
    };
    let mut config = Config::new(&public_url, &key);
    config.trust_proxy = env("RELAY_TRUST_PROXY").is_some_and(|v| v == "1");
    if let Some(max) = env("RELAY_MAX_JAMS").and_then(|v| v.parse().ok()) {
        config.max_jams = max;
    }
    if let Some(max) = env("RELAY_MAX_JAMS_PER_ADDRESS").and_then(|v| v.parse().ok()) {
        config.max_jams_per_address = max;
    }
    let bind = env("RELAY_BIND").unwrap_or_else(|| "0.0.0.0:8080".into());
    let listener = match TcpListener::bind(&bind).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("cannot listen on {bind}: {e}");
            return ExitCode::FAILURE;
        }
    };
    tracing::info!("relay on {bind}, public address {}", config.public_url);
    match serve(listener, config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("relay stopped: {e}");
            ExitCode::FAILURE
        }
    }
}
