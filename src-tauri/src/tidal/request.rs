//! Authenticated requests to the TIDAL API: `countryCode`, token refreshed on a 401, pause
//! shared after a 429.

use std::time::Instant;

use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use super::api::{API_V1, TidalClient};
use super::backoff::{self, Gate};
use super::{Result, TidalError};
use crate::i18n::trf;

const JSON_API: &str = "application/vnd.api+json";
const CLIENT_VERSION: &str = "2025.7.16";

impl TidalClient {
    /// GET on the v1 API with `countryCode`, refreshing the token on a 401.
    pub(super) async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<T> {
        self.get_at(API_V1, path, params).await
    }

    pub(super) async fn get_at<T: DeserializeOwned>(
        &self,
        base: &str,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<T> {
        self.call(&Call::get(base, path, params))
            .await?
            .json()
            .await
            .map_err(|e| TidalError::Decode(format!("{path}: {e}")))
    }

    /// Authenticated request with `countryCode`, refreshing the token on a 401.
    pub(super) async fn call(&self, call: &Call<'_>) -> Result<reqwest::Response> {
        let mut rejected: Option<String> = None;
        let mut retried = false;
        loop {
            self.wait_backoff().await?;
            let (access, country) = self.credentials(rejected.as_deref()).await?;
            let mut query: Vec<(&str, String)> = vec![("countryCode", country)];
            query.extend(call.query.iter().cloned());
            let resp = self
                .send(
                    &access,
                    &Call {
                        query: &query,
                        ..call.clone()
                    },
                )
                .await;
            match resp {
                Err(TidalError::Api { status: 401, .. }) if rejected.is_none() => {
                    rejected = Some(access);
                }
                // One more try after the shared pause (#37).
                Err(TidalError::Api { status: 429, .. }) if !retried => retried = true,
                other => return other,
            }
        }
    }

    /// Waits for the pause after a 429, or fails at once if it is too long (#37).
    async fn wait_backoff(&self) -> Result<()> {
        match self.backoff.gate(Instant::now()) {
            Gate::Go => Ok(()),
            Gate::Wait(left) => {
                tokio::time::sleep(left).await;
                Ok(())
            }
            Gate::Refuse(left) => Err(TidalError::Api {
                status: 429,
                message: trf!(
                    "trop de requêtes, nouvel essai possible dans {} s",
                    "too many requests, try again in {} s",
                    left.as_secs().max(1)
                ),
            }),
        }
    }

    pub(super) async fn send(&self, access: &str, call: &Call<'_>) -> Result<reqwest::Response> {
        let mut req = self
            .http
            .request(call.method.clone(), format!("{}{}", call.base, call.path))
            .query(call.query)
            .bearer_auth(access)
            .header("x-tidal-client-version", CLIENT_VERSION);
        if let Some(form) = call.form {
            req = req.form(form);
        }
        if let Some(json) = call.json {
            // JSON:API of `openapi.tidal.com` (#196): any other content type is refused there.
            req = req
                .header(reqwest::header::CONTENT_TYPE, JSON_API)
                .body(json.to_string());
        }
        if let Some(etag) = call.if_none_match {
            req = req.header("If-None-Match", etag);
        }
        for (name, value) in call.headers {
            req = req.header(*name, *value);
        }
        let resp = req.send().await?;
        let path = call.path;
        let status = resp.status();
        if status.is_success() {
            self.backoff.success();
            return Ok(resp);
        }
        if status == StatusCode::TOO_MANY_REQUESTS {
            let retry_after = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(backoff::parse_retry_after);
            let pause = self.backoff.hit(retry_after, Instant::now());
            tracing::warn!("429 on {path}, requests paused for {} s", pause.as_secs());
        }
        let message = resp
            .json::<RawApiError>()
            .await
            .ok()
            .and_then(RawApiError::message)
            .unwrap_or_else(|| status.canonical_reason().unwrap_or("").to_owned());
        Err(TidalError::Api {
            status: status.as_u16(),
            message: if status == StatusCode::NOT_FOUND {
                format!("{path} not found ({message})")
            } else {
                message
            },
        })
    }
}

/// A request to the API: GET by default, or a write with a form and an ETag.
#[derive(Clone)]
pub(super) struct Call<'a> {
    pub method: Method,
    pub base: &'a str,
    pub path: &'a str,
    pub query: &'a [(&'a str, String)],
    /// `application/x-www-form-urlencoded` body.
    pub form: Option<&'a [(&'a str, String)]>,
    /// JSON:API body (`application/vnd.api+json`), for `openapi.tidal.com`.
    pub json: Option<&'a serde_json::Value>,
    /// ETag of the resource: TIDAL refuses a playlist change without it.
    pub if_none_match: Option<&'a str>,
    /// Extra headers (streaming session of a stream request, #34).
    pub headers: &'a [(&'a str, &'a str)],
}

impl<'a> Call<'a> {
    pub fn get(base: &'a str, path: &'a str, query: &'a [(&'a str, String)]) -> Self {
        Self {
            method: Method::GET,
            base,
            path,
            query,
            form: None,
            json: None,
            if_none_match: None,
            headers: &[],
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawApiError {
    user_message: Option<String>,
    /// Error format of the v2 API.
    #[serde(default)]
    errors: Vec<RawApiErrorDetail>,
}

#[derive(Deserialize)]
struct RawApiErrorDetail {
    detail: Option<String>,
}

impl RawApiError {
    fn message(self) -> Option<String> {
        self.user_message
            .or_else(|| self.errors.into_iter().find_map(|e| e.detail))
    }
}
