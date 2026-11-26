//! Session TIDAL : restauration, connexion, déconnexion et rafraîchissement du token.

use std::time::{Duration, Instant};

use reqwest::Url;
use serde::Deserialize;

use super::api::{API_V1, TidalClient};
use super::auth::{self, PkceChallenge, Tokens};
use super::models::SessionInfo;
use super::request::Call;
use super::{Result, TidalError};

/// Marge avant expiration à partir de laquelle on rafraîchit l'access token.
const REFRESH_MARGIN: Duration = Duration::from_secs(60);

pub(super) struct AuthState {
    access: String,
    expires_at: Instant,
    refresh: String,
    session: SessionInfo,
}

impl TidalClient {
    /// Session courante, en restaurant au premier appel celle du refresh token stocké.
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
            // Refresh token révoqué ou expiré : on l'oublie, l'UI proposera la connexion.
            Err(TidalError::Auth(reason)) => {
                tracing::warn!("refresh token refusé ({reason}), reconnexion nécessaire");
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
            .ok_or_else(|| TidalError::Auth("pas de refresh token dans la réponse".into()))?;
        auth::store_refresh_token(&refresh)?;
        self.install(tokens, refresh).await
    }

    pub async fn logout(&self) -> Result<()> {
        *self.auth.write().await = None;
        self.forget_account();
        auth::clear_refresh_token()
    }

    /// Caches propres au compte, vidés à la déconnexion et quand la session est perdue.
    fn forget_account(&self) {
        self.likes.clear();
        self.later.clear();
        self.blocks.clear();
        self.availability.clear();
        self.playlists.clear();
        self.versions.clear();
    }

    /// Attend que la session soit perdue en cours de route : TIDAL a refusé le
    /// rafraîchissement, l'utilisateur doit se reconnecter (#259). Un signal émis sans
    /// personne pour l'attendre est gardé pour le prochain appel.
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
            .map_err(|e| TidalError::Decode(format!("sessions : {e}")))?;
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

    /// Access token valide et code pays, en rafraîchissant s'il expire ou si `rejected`
    /// (token refusé par un 401) est encore le token courant.
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
        // Les requêtes qui attendaient ce verrou trouvent le token déjà rafraîchi par la
        // première : un seul rafraîchissement par expiration ou par 401 (#148).
        if usable(state, rejected, Instant::now()) {
            return Ok((state.access.clone(), state.session.country_code.clone()));
        }
        let tokens = match auth::refresh(&self.http, &self.creds, &state.refresh).await {
            Ok(tokens) => tokens,
            // Refresh token révoqué ou expiré : sans ça, l'état resterait « connecté » et
            // chaque requête retenterait un rafraîchissement voué à l'échec (#259).
            Err(TidalError::Auth(reason)) => {
                tracing::warn!("rafraîchissement refusé ({reason}), reconnexion nécessaire");
                *guard = None;
                self.forget_account();
                if let Err(e) = auth::clear_refresh_token() {
                    tracing::warn!("refresh token non effacé : {e}");
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

/// Le token courant peut servir : il n'expire pas avant la marge et n'est pas celui qu'un
/// 401 vient de refuser (sinon, déjà remplacé par une autre requête).
fn usable(state: &AuthState, rejected: Option<&str>, now: Instant) -> bool {
    rejected != Some(state.access.as_str()) && state.expires_at > now + REFRESH_MARGIN
}

/// Garde le refresh token renvoyé par un rafraîchissement s'il remplace l'ancien. TIDAL n'en
/// renvoie pas aujourd'hui (#252) ; s'il se mettait à le faire en invalidant l'ancien, l'app
/// perdrait sinon sa session au lancement suivant.
fn keep_rotated(current: &mut String, new: Option<String>) {
    let Some(new) = new.filter(|n| n != current) else {
        return;
    };
    if let Err(e) = auth::store_refresh_token(&new) {
        tracing::warn!("nouveau refresh token non enregistré : {e}");
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

    /// Une requête qui attendait le verrou réutilise le token rafraîchi par une autre (#148).
    #[test]
    fn a_refreshed_token_is_reused() {
        let now = Instant::now();
        let fresh = state("new", Duration::from_secs(4 * 3600), now);
        assert!(usable(&fresh, None, now));
        // 401 sur l'ancien token, déjà remplacé : pas de nouveau rafraîchissement.
        assert!(usable(&fresh, Some("old"), now));
        // 401 sur le token courant : il faut rafraîchir.
        assert!(!usable(&fresh, Some("new"), now));
        // Expiré ou dans la marge de 60 s : il faut rafraîchir.
        assert!(!usable(
            &state("new", Duration::from_secs(30), now),
            None,
            now
        ));
    }
}
