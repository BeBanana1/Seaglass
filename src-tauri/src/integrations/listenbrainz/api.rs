//! Requests to the ListenBrainz API: token validation and listen submission.
//!
//! The token only leaves this module in the `Authorization` header: never in a log or an error
//! message.

use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;

use super::payload::{ListenType, submission};
use crate::i18n::{tr, trf};

const API: &str = "https://api.listenbrainz.org/1";
const TIMEOUT: Duration = Duration::from_secs(20);

/// User token, without `Debug` so that it never ends up in a log.
#[derive(Clone, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    /// Token pasted by the user, whitespace removed; `None` if it is empty or contains characters
    /// an HTTP header would refuse.
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

/// A failed submission, and what to do about it.
#[derive(Debug, PartialEq, Eq)]
pub enum SendError {
    /// Network, service unavailable, too many requests: retry (after the delay the service gives,
    /// if any).
    Retry(Option<Duration>),
    /// Token refused: send nothing more until a new token.
    Unauthorized,
    /// Listen refused (400): sending it again as is would fail again.
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

/// Account name of the token. ListenBrainz answers 200 even to an invalid token (`"valid": false`).
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
            tracing::info!("ListenBrainz unreachable: {}", e.without_url());
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
                "ListenBrainz: submission refused ({status}): {}",
                body.chars().take(300).collect::<String>()
            );
            Err(e)
        }
    }
}

/// What to do with a response, depending on its status.
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
