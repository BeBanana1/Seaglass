//! Tout ce qui touche à l'API privée TIDAL. Le reste de l'app ne dépend que de `models`.

pub mod api;
pub mod auth;
pub mod cache;
mod catalog;
mod collection;
mod date;
mod feed;
mod hires;
mod likes;
#[cfg(test)]
mod live_tests;
mod lyrics;
mod manifest;
mod mix;
pub mod models;
mod raw;

pub use api::TidalClient;

#[derive(Debug, thiserror::Error)]
pub enum TidalError {
    #[error("pas de session TIDAL, connexion requise")]
    NotLoggedIn,
    #[error("identifiant client absent : définir TIDAL_CLIENT_ID dans .env")]
    MissingClientId,
    #[error("connexion annulée")]
    LoginCancelled,
    #[error("authentification refusée : {0}")]
    Auth(String),
    #[error("erreur API TIDAL {status} : {message}")]
    Api { status: u16, message: String },
    #[error("réseau : {0}")]
    Http(#[from] reqwest::Error),
    #[error("réponse inattendue : {0}")]
    Decode(String),
    /// La ressource a changé depuis sa lecture (ETag périmé, titre déplacé).
    #[error("{0}")]
    Conflict(String),
    #[error("flux protégé par DRM ({0}), lecture impossible")]
    Drm(String),
    #[error("format de flux non pris en charge : {0}")]
    UnsupportedStream(String),
    #[error("Gestionnaire d'identifiants : {0}")]
    Keyring(#[from] keyring::Error),
}

pub type Result<T> = std::result::Result<T, TidalError>;
