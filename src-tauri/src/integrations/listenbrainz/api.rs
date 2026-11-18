//! Requêtes à l'API de ListenBrainz : vérification du jeton et envoi des écoutes.
//!
//! Le jeton ne sort de ce module que dans l'en-tête `Authorization` : jamais dans un
//! journal ni dans un message d'erreur.

use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;

use super::payload::{ListenType, submission};
use crate::i18n::{tr, trf};

const API: &str = "https://api.listenbrainz.org/1";
const TIMEOUT: Duration = Duration::from_secs(20);

/// Jeton utilisateur, sans `Debug` pour qu'il ne finisse pas dans un journal.
#[derive(Clone, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    /// Jeton collé par l'utilisateur, espaces retirés ; `None` s'il est vide ou contient
    /// des caractères qu'un en-tête HTTP refuserait.
    pub fn parse(raw: &str) -> Option<Self> {
        let token = raw.trim();
        let valid =
            !token.is_empty() && token.len() <= 256 && token.chars().all(|c| c.is_ascii_graphic());
        valid.then(|| Self(token.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn header(&self) -> String {
        format!("Token {}", self.0)
    }
}

/// Échec d'un envoi, et ce qu'il faut en faire.
#[derive(Debug, PartialEq, Eq)]
pub enum SendError {
    /// Réseau, service indisponible, trop de requêtes : réessayer (après le délai donné
    /// par le service, s'il en donne un).
    Retry(Option<Duration>),
    /// Jeton refusé : ne plus rien envoyer avant un nouveau jeton.
    Unauthorized,
    /// Écoute refusée (400) : la renvoyer telle quelle échouerait encore.
    Rejected,
}

pub fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .https_only(true)
        .timeout(TIMEOUT)
        .user_agent(concat!("Seaglass/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())
}

#[derive(Deserialize)]
struct Validation {
    #[serde(default)]
    valid: bool,
    #[serde(default)]
    user_name: Option<String>,
}

/// Nom du compte du jeton. ListenBrainz répond 200 même à un jeton invalide
/// (`"valid": false`).
pub async fn validate(client: &reqwest::Client, token: &Token) -> Result<String, String> {
    let response = client
        .get(format!("{API}/validate-token"))
        .header("Authorization", token.header())
        .send()
        .await
        .map_err(|e| unreachable(&e))?;
    let status = response.status();
    if !status.is_success() {
        return Err(trf!(
            "ListenBrainz ne répond pas ({})",
            "ListenBrainz is not responding ({})",
            status.as_u16()
        ));
    }
    let body: Validation = response.json().await.map_err(|e| unreachable(&e))?;
    match body.user_name {
        Some(user) if body.valid => Ok(user),
        _ => Err(tr(
            "jeton refusé par ListenBrainz : copiez-le depuis vos réglages ListenBrainz",
            "token refused by ListenBrainz: copy it from your ListenBrainz settings",
        )
        .to_owned()),
    }
}

pub async fn submit(
    client: &reqwest::Client,
    token: &Token,
    kind: ListenType,
    listens: Vec<Value>,
) -> Result<(), SendError> {
    let response = client
        .post(format!("{API}/submit-listens"))
        .header("Authorization", token.header())
        .json(&submission(kind, listens))
        .send()
        .await
        .map_err(|e| {
            tracing::info!("ListenBrainz injoignable : {}", e.without_url());
            SendError::Retry(None)
        })?;
    let status = response.status().as_u16();
    let reset_in = response
        .headers()
        .get("X-RateLimit-Reset-In")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(Duration::from_secs);
    match classify(status, reset_in) {
        Ok(()) => Ok(()),
        Err(e) => {
            let body = response.text().await.unwrap_or_default();
            tracing::info!(
                "ListenBrainz : envoi refusé ({status}) : {}",
                body.chars().take(300).collect::<String>()
            );
            Err(e)
        }
    }
}

/// Que faire d'une réponse, selon son statut.
fn classify(status: u16, reset_in: Option<Duration>) -> Result<(), SendError> {
    match status {
        200..=299 => Ok(()),
        401 | 403 => Err(SendError::Unauthorized),
        429 => Err(SendError::Retry(reset_in)),
        400 | 413 => Err(SendError::Rejected),
        _ => Err(SendError::Retry(None)),
    }
}

fn unreachable(e: &reqwest::Error) -> String {
    trf!(
        "ListenBrainz injoignable ({})",
        "ListenBrainz unreachable ({})",
        e
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_parsing() {
        assert!(Token::parse("  ").is_none());
        assert!(Token::parse("abc def").is_none());
        assert_eq!(
            Token::parse(" 1234-abcd \n").map(|t| t.as_str().to_owned()),
            Some("1234-abcd".into())
        );
    }

    #[test]
    fn statuses() {
        assert_eq!(classify(200, None), Ok(()));
        assert_eq!(classify(401, None), Err(SendError::Unauthorized));
        assert_eq!(classify(400, None), Err(SendError::Rejected));
        let wait = Some(Duration::from_secs(5));
        assert_eq!(classify(429, wait), Err(SendError::Retry(wait)));
        assert_eq!(classify(503, wait), Err(SendError::Retry(None)));
    }
}
