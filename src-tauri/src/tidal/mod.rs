//! Tout ce qui touche à l'API privée TIDAL. Le reste de l'app ne dépend que de `models`.

pub mod api;
pub mod auth;
pub mod cache;
mod catalog;
mod collection;
mod date;
mod feed;
mod hires;
mod info;
mod likes;
#[cfg(test)]
mod live_collection;
#[cfg(test)]
mod live_tests;
mod lyrics;
mod manifest;
mod mix;
pub mod models;
mod raw;
mod twin;

pub use api::TidalClient;

use crate::i18n::{tr, trf};

#[derive(Debug, thiserror::Error)]
pub enum TidalError {
    #[error(
        "{}",
        tr(
            "pas de session TIDAL, connexion requise",
            "no TIDAL session, please sign in"
        )
    )]
    NotLoggedIn,
    #[error(
        "{}",
        tr(
            "identifiant client absent : définir TIDAL_CLIENT_ID dans .env",
            "missing client ID: set TIDAL_CLIENT_ID in .env"
        )
    )]
    MissingClientId,
    #[error("{}", tr("connexion annulée", "sign-in cancelled"))]
    LoginCancelled,
    #[error("{}", trf!("authentification refusée : {}", "authentication refused: {}", .0))]
    Auth(String),
    #[error("{}", trf!("erreur API TIDAL {} : {}", "TIDAL API error {}: {}", .status, .message))]
    Api { status: u16, message: String },
    #[error("{}", trf!("réseau : {}", "network: {}", .0))]
    Http(#[from] reqwest::Error),
    #[error("{}", trf!("réponse inattendue : {}", "unexpected response: {}", .0))]
    Decode(String),
    /// La ressource a changé depuis sa lecture (ETag périmé, titre déplacé). Message déjà
    /// dans la langue courante.
    #[error("{0}")]
    Conflict(String),
    #[error("{}", trf!(
        "flux protégé par DRM ({}), lecture impossible",
        "DRM-protected stream ({}), cannot play",
        .0
    ))]
    Drm(String),
    #[error("{}", trf!("format de flux non pris en charge : {}", "unsupported stream format: {}", .0))]
    UnsupportedStream(String),
    #[error("{}", trf!("Gestionnaire d'identifiants : {}", "Credential Manager: {}", .0))]
    Keyring(#[from] keyring::Error),
}

pub type Result<T> = std::result::Result<T, TidalError>;
