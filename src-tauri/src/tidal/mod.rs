//! Tout ce qui touche à l'API privée TIDAL. Le reste de l'app ne dépend que de `models`.

pub mod api;
pub mod auth;
pub mod cache;
mod catalog;
mod collection;
mod credits;
mod date;
mod feed;
mod hires;
pub mod home;
mod info;
mod likes;
#[cfg(test)]
mod live_collection;
#[cfg(test)]
mod live_playlist_cache;
#[cfg(test)]
mod live_tests;
mod lyrics;
mod manifest;
mod mix;
pub mod models;
mod paging;
mod playlist;
mod raw;
mod twin;

pub use api::TidalClient;
pub use catalog::LIBRARY_MAX;

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
    /// Toujours sans URL (voir `From<reqwest::Error>`).
    #[error("{}", trf!("réseau : {}", "network: {}", .0))]
    Http(reqwest::Error),
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

/// L'URL est retirée de l'erreur : celles des segments sont signées (`?token=…`) et le
/// message finit dans les journaux et dans les toasts (#144).
impl From<reqwest::Error> for TidalError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(e.without_url())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::TidalError;

    /// Une erreur réseau sur une URL signée ne garde ni l'URL ni sa signature (#144).
    #[tokio::test]
    async fn network_errors_drop_the_signed_url() {
        let raw = reqwest::Client::new()
            .get("http://127.0.0.1:9/segment.mp4?token=secret-signature")
            .timeout(Duration::from_millis(1))
            .send()
            .await
            .expect_err("délai de 1 ms");
        assert!(raw.to_string().contains("secret-signature"), "{raw}");
        let message = TidalError::from(raw).to_string();
        assert!(!message.contains("secret-signature"), "{message}");
        assert!(!message.contains("127.0.0.1"), "{message}");
    }
}
