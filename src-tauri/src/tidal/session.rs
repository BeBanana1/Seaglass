//! TIDAL session: restore, sign-in, sign-out and access token refresh.

use std::time::{Duration, Instant};

use reqwest::Url;
use serde::Deserialize;

use super::api::{API_V1, TidalClient};
use super::auth::{self, PkceChallenge, Tokens};
use super::models::SessionInfo;
use super::request::Call;
use super::{Result, TidalError};

/// Margin before expiry from which the access token is refreshed.
const REFRESH_MARGIN: Duration = Duration::from_secs(60);

pub(super) struct AuthState {
    access: String,
    expires_at: Instant,
    refresh: String,
    session: SessionInfo,
}

impl TidalClient {
    /// Current session, restoring the stored refresh token's one on the first call.
    pub async fn current_session(&self) -> Result<Option<SessionInfo>> {
        self.restored
            .get_or_try_init(|| async { self.restore().await })
            .await?;
        Ok(self.session().await)
    }

    async fn restore(&self) -> Result<()> {
        let Some(refresh) = auth::load_refresh_token()? else {
            return Ok(());
        };
        match auth::refresh(&self.http, &self.creds, &refresh).await {
            Ok(mut tokens) => {
                let mut refresh = refresh;
                keep_rotated(&mut refresh, tokens.refresh.take());
                self.install(tokens, refresh).await.map(drop)
            }
            // Refresh token revoked or expired: it is forgotten, the UI will offer to sign in.
            Err(TidalError::Auth(reason)) => {
                tracing::warn!("refresh token refused ({reason}), sign-in needed");
                auth::clear_refresh_token()
            }
            Err(e) => Err(e),
        }
    }

    pub fn begin_login(&self) -> Result<(PkceChallenge, Url)> {
        let challenge = PkceChallenge::new();
        let url = challenge.authorize_url(&self.creds)?;
        Ok((challenge, url))
    }

    pub async fn complete_login(
        &self,
        challenge: &PkceChallenge,
        code: &str,
    ) -> Result<SessionInfo> {
        let mut tokens = auth::exchange_code(&self.http, &self.creds, challenge, code).await?;
        let refresh = tokens
            .refresh
            .take()
            .ok_or_else(|| TidalError::Auth("no refresh token in the answer".into()))?;
        auth::store_refresh_token(&refresh)?;
        self.install(tokens, refresh).await
    }

    pub async fn logout(&self) -> Result<()> {
        *self.auth.write().await = None;
        self.forget_account();
        auth::clear_refresh_token()
    }

    /// Caches tied to the account, cleared on sign-out and when the session is lost.
    fn forget_account(&self) {
        self.likes.clear();
        self.later.clear();
        self.blocks.clear();
        self.availability.clear();
        self.playlists.clear();
        self.versions.clear();
    }

    /// Waits for the session to be lost along the way: TIDAL refused the refresh, the user
    /// must sign in again (#259). A signal sent with no one waiting is kept for the next
    /// call.
    pub async fn session_lost(&self) {
        self.session_lost.notified().await;
    }

    pub async fn session(&self) -> Option<SessionInfo> {
        self.auth.read().await.as_ref().map(|a| a.session.clone())
    }

    async fn install(&self, tokens: Tokens, refresh: String) -> Result<SessionInfo> {
        let raw: RawSession = self
            .send(&tokens.access, &Call::get(API_V1, "sessions", &[]))
            .await?
            .json()
            .await
            .map_err(|e| TidalError::Decode(format!("sessions: {e}")))?;
        let session = SessionInfo {
            user_id: raw.user_id,
            country_code: raw.country_code,
        };
        *self.auth.write().await = Some(AuthState {
            access: tokens.access,
            expires_at: tokens.expires_at,
            refresh,
            session: session.clone(),
        });
        Ok(session)
    }

    /// Valid access token and country code, refreshing if it expires or if `rejected` (a
    /// token refused by a 401) is still the current token.
    pub(super) async fn credentials(&self, rejected: Option<&str>) -> Result<(String, String)> {
        {
            let guard = self.auth.read().await;
            let state = guard.as_ref().ok_or(TidalError::NotLoggedIn)?;
            if usable(state, rejected, Instant::now()) {
                return Ok((state.access.clone(), state.session.country_code.clone()));
            }
        }
        let mut guard = self.auth.write().await;
        let state = guard.as_mut().ok_or(TidalError::NotLoggedIn)?;
        // Requests that waited for this lock find the token already refreshed by the first
        // one: a single refresh per expiry or per 401 (#148).
        if usable(state, rejected, Instant::now()) {
            return Ok((state.access.clone(), state.session.country_code.clone()));
        }
        let tokens = match auth::refresh(&self.http, &self.creds, &state.refresh).await {
            Ok(tokens) => tokens,
            // Refresh token revoked or expired: otherwise the state would stay "signed in"
            // and every request would retry a refresh bound to fail (#259).
            Err(TidalError::Auth(reason)) => {
                tracing::warn!("refresh refused ({reason}), sign-in needed");
                *guard = None;
                self.forget_account();
                if let Err(e) = auth::clear_refresh_token() {
                    tracing::warn!("refresh token not cleared: {e}");
                }
                self.session_lost.notify_one();
                return Err(TidalError::NotLoggedIn);
            }
            Err(e) => return Err(e),
        };
        keep_rotated(&mut state.refresh, tokens.refresh);
        state.access = tokens.access;
        state.expires_at = tokens.expires_at;
        Ok((state.access.clone(), state.session.country_code.clone()))
    }
}

/// The current token can be used: it does not expire within the margin and is not the one
/// a 401 just refused (otherwise, already replaced by another request).
fn usable(state: &AuthState, rejected: Option<&str>, now: Instant) -> bool {
    rejected != Some(state.access.as_str()) && state.expires_at > now + REFRESH_MARGIN
}

/// Keeps the refresh token returned by a refresh if it replaces the old one. TIDAL returns
/// none today (#252); if it started to while invalidating the old one, the app would
/// otherwise lose its session at the next launch.
fn keep_rotated(current: &mut String, new: Option<String>) {
    let Some(new) = new.filter(|n| n != current) else {
        return;
    };
    if let Err(e) = auth::store_refresh_token(&new) {
        tracing::warn!("new refresh token not saved: {e}");
    }
    *current = new;
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSession {
    user_id: u64,
    country_code: String,
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{AuthState, usable};
    use crate::tidal::models::SessionInfo;

    fn state(access: &str, expires_in: Duration, now: Instant) -> AuthState {
        AuthState {
            access: access.into(),
            expires_at: now + expires_in,
            refresh: "r".into(),
            session: SessionInfo {
                user_id: 1,
                country_code: "CH".into(),
            },
        }
    }

    /// A request that waited for the lock reuses the token another one refreshed (#148).
    #[test]
    fn a_refreshed_token_is_reused() {
        let now = Instant::now();
        let fresh = state("new", Duration::from_secs(4 * 3600), now);
        assert!(usable(&fresh, None, now));
        // 401 on the old token, already replaced: no new refresh.
        assert!(usable(&fresh, Some("old"), now));
        // 401 on the current token: it must be refreshed.
        assert!(!usable(&fresh, Some("new"), now));
        // Expired or within the 60 s margin: it must be refreshed.
        assert!(!usable(
            &state("new", Duration::from_secs(30), now),
            None,
            now
        ));
    }
}
