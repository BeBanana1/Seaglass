//! PKCE sign-in (the only flow that gives FLAC, see the spike results) and storage of the
//! refresh token in the Windows Credential Manager.
//!
//! No token is ever logged, even truncated.

use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use reqwest::{StatusCode, Url};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{Result, TidalError};

pub const REDIRECT_URI: &str = "https://tidal.com/android/login/auth";
const AUTHORIZE_URL: &str = "https://login.tidal.com/authorize";
const TOKEN_URL: &str = "https://auth.tidal.com/v1/oauth2/token";
const SCOPE: &str = "r_usr w_usr w_sub";

const KEYRING_SERVICE: &str = "seaglass";
const KEYRING_USER: &str = "tidal-refresh-token";

/// Client id: environment variable at launch, otherwise the `.env` value
/// injected at compile time by `build.rs`. Deliberately without `Debug`.
pub struct ClientCredentials {
    pub id: String,
    secret: Option<String>,
}

impl ClientCredentials {
    pub fn load() -> Result<Self> {
        let read = |runtime: &str, embedded: Option<&'static str>| {
            std::env::var(runtime)
                .ok()
                .or_else(|| embedded.map(str::to_owned))
                .filter(|v| !v.is_empty())
        };
        let id = read("TIDAL_CLIENT_ID", option_env!("SEAGLASS_TIDAL_CLIENT_ID"))
            .ok_or(TidalError::MissingClientId)?;
        let secret = read(
            "TIDAL_CLIENT_SECRET",
            option_env!("SEAGLASS_TIDAL_CLIENT_SECRET"),
        );
        Ok(Self { id, secret })
    }
}

#[derive(Clone)]
pub struct PkceChallenge {
    verifier: String,
    challenge: String,
    unique_key: String,
}

impl PkceChallenge {
    pub fn new() -> Self {
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes);
        let verifier = URL_SAFE_NO_PAD.encode(bytes);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let unique_key = format!("{:016x}", rand::rng().next_u64());
        Self {
            verifier,
            challenge,
            unique_key,
        }
    }

    pub fn authorize_url(&self, creds: &ClientCredentials) -> Result<Url> {
        Url::parse_with_params(
            AUTHORIZE_URL,
            [
                ("response_type", "code"),
                ("redirect_uri", REDIRECT_URI),
                ("client_id", creds.id.as_str()),
                ("lang", "EN"),
                ("appMode", "android"),
                ("client_unique_key", self.unique_key.as_str()),
                ("code_challenge", self.challenge.as_str()),
                ("code_challenge_method", "S256"),
                ("restrict_signup", "true"),
            ],
        )
        .map_err(|e| TidalError::Decode(format!("sign-in URL: {e}")))
    }
}

/// Extracts the authorization code if `url` is the redirect that ends the sign-in.
pub fn redirect_code(url: &Url) -> Option<String> {
    if !url.as_str().starts_with(REDIRECT_URI) {
        return None;
    }
    url.query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
}

pub struct Tokens {
    pub access: String,
    pub expires_at: Instant,
    /// Present after the code exchange. A refresh does not return one (checked on
    /// 2026-11-10, `live_refresh_rotation`); if it did, the app would keep it.
    pub refresh: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
}

#[derive(Deserialize, Default)]
struct TokenErrorBody {
    error: Option<String>,
    error_description: Option<String>,
}

pub async fn exchange_code(
    http: &reqwest::Client,
    creds: &ClientCredentials,
    challenge: &PkceChallenge,
    code: &str,
) -> Result<Tokens> {
    token_request(
        http,
        &[
            ("code", code),
            ("client_id", &creds.id),
            ("grant_type", "authorization_code"),
            ("redirect_uri", REDIRECT_URI),
            ("scope", SCOPE),
            ("code_verifier", &challenge.verifier),
            ("client_unique_key", &challenge.unique_key),
        ],
    )
    .await
}

pub async fn refresh(
    http: &reqwest::Client,
    creds: &ClientCredentials,
    refresh_token: &str,
) -> Result<Tokens> {
    let mut form = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", creds.id.as_str()),
    ];
    if let Some(secret) = &creds.secret {
        form.push(("client_secret", secret));
    }
    token_request(http, &form).await
}

async fn token_request(http: &reqwest::Client, form: &[(&str, &str)]) -> Result<Tokens> {
    let resp = http.post(TOKEN_URL).form(form).send().await?;
    let status = resp.status();
    if !status.is_success() {
        let body: TokenErrorBody = resp.json().await.unwrap_or_default();
        let message = format!(
            "{} {}",
            body.error.unwrap_or_default(),
            body.error_description.unwrap_or_default()
        );
        return Err(token_error(status, message.trim().to_owned()));
    }
    let token: TokenResponse = resp
        .json()
        .await
        .map_err(|e| TidalError::Decode(format!("token response: {e}")))?;
    Ok(Tokens {
        access: token.access_token,
        expires_at: Instant::now() + Duration::from_secs(token.expires_in),
        refresh: token.refresh_token,
    })
}

/// Only an explicit refusal (400 `invalid_grant`, 401, 403) invalidates the session. A server
/// outage or a rate limit (5xx, 429) is temporary: treated as a refusal, it used to
/// erase the refresh token and force a new sign-in (#252).
fn token_error(status: StatusCode, message: String) -> TidalError {
    match status.as_u16() {
        400 | 401 | 403 => TidalError::Auth(format!("{status} {message}")),
        code => TidalError::Api {
            status: code,
            message,
        },
    }
}

fn keyring_entry() -> Result<keyring::Entry> {
    Ok(keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)?)
}

pub fn store_refresh_token(token: &str) -> Result<()> {
    Ok(keyring_entry()?.set_password(token)?)
}

pub fn load_refresh_token() -> Result<Option<String>> {
    match keyring_entry()?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn clear_refresh_token() -> Result<()> {
    match keyring_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_is_sha256_of_verifier() {
        let c = PkceChallenge::new();
        assert_eq!(c.verifier.len(), 43);
        let expected = URL_SAFE_NO_PAD.encode(Sha256::digest(c.verifier.as_bytes()));
        assert_eq!(c.challenge, expected);
        assert_eq!(c.unique_key.len(), 16);
    }

    #[test]
    fn only_explicit_refusals_end_the_session() {
        for code in [400, 401, 403] {
            let status = StatusCode::from_u16(code).unwrap();
            assert!(matches!(
                token_error(status, String::new()),
                TidalError::Auth(_)
            ));
        }
        for code in [429, 500, 502, 503] {
            let status = StatusCode::from_u16(code).unwrap();
            assert!(matches!(
                token_error(status, String::new()),
                TidalError::Api { status, .. } if status == code
            ));
        }
    }

    #[test]
    fn redirect_code_only_on_redirect_uri() {
        let ok = Url::parse("https://tidal.com/android/login/auth?code=abc123&state=x").unwrap();
        assert_eq!(redirect_code(&ok).as_deref(), Some("abc123"));
        let other = Url::parse("https://login.tidal.com/authorize?code=abc123").unwrap();
        assert_eq!(redirect_code(&other), None);
    }
}
