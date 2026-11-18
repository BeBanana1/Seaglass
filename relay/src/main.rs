//! Relais de la Jam Seaglass. Configuration par variables d'environnement :
//!
//! - `SEAGLASS_RELAY_KEY` (obligatoire) : clé de build de l'app, la même que son `.env`.
//! - `RELAY_PUBLIC_URL` (obligatoire) : adresse publique, par exemple `https://jam.example.org`.
//! - `RELAY_BIND` : adresse d'écoute (défaut `0.0.0.0:8080`).
//! - `RELAY_TRUST_PROXY` : `1` derrière Caddy (lit `X-Forwarded-For`), défaut `0`.
//! - `RELAY_MAX_JAMS` : Jams ouvertes en même temps (défaut 20).
//! - `RELAY_MAX_JAMS_PER_ADDRESS` : dont celles d'une même adresse d'hôte (défaut 3).

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
        tracing::error!("SEAGLASS_RELAY_KEY manquante");
        return ExitCode::FAILURE;
    };
    let Some(public_url) = env("RELAY_PUBLIC_URL") else {
        tracing::error!("RELAY_PUBLIC_URL manquante (adresse publique du relais)");
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
            tracing::error!("écoute impossible sur {bind} : {e}");
            return ExitCode::FAILURE;
        }
    };
    tracing::info!("relais sur {bind}, adresse publique {}", config.public_url);
    match serve(listener, config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("relais arrêté : {e}");
            ExitCode::FAILURE
        }
    }
}
