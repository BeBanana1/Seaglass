//! Client HTTP TIDAL : authentification, requêtes, titres et flux.
//! Les endpoints de catalogue (albums, artistes, playlists, favoris, accueil) sont dans
//! `catalog.rs` ; les formats JSON bruts dans `raw.rs`.
//!
//! Endpoints vérifiés avec le spike : `sessions`, `tracks/{id}`,
//! `tracks/{id}/playbackinfopostpaywall`.

use std::time::{Duration, Instant};

use reqwest::{Method, StatusCode, Url};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use tokio::sync::{Notify, OnceCell, RwLock};

use super::auth::{self, ClientCredentials, PkceChallenge, Tokens};
use super::backoff::{self, Backoff, Gate};
use super::cache::TrackCache;
use super::hires::HiresCache;
use super::likes::Likes;
use super::manifest;
use super::models::{AudioQuality, ReplayGain, SessionInfo, Stream, StreamFormat, Track, TrackId};
use super::playlist::PlaylistCache;
use super::raw::RawTrack;
use super::{Result, TidalError};
use crate::i18n::trf;

pub(super) const API_V1: &str = "https://api.tidal.com/v1/";
pub(super) const API_V2: &str = "https://api.tidal.com/v2/";
/// API officielle (JSON:API), acceptée avec le token du client PKCE (vérifié le
/// 2026-11-05, #196).
pub(super) const OPEN_API: &str = "https://openapi.tidal.com/v2/";
const JSON_API: &str = "application/vnd.api+json";
const USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 12; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/91.0.4472.114 Safari/537.36";
const CLIENT_VERSION: &str = "2025.7.16";
/// Marge avant expiration à partir de laquelle on rafraîchit l'access token.
const REFRESH_MARGIN: Duration = Duration::from_secs(60);
/// Taille maximale lue dans un segment d'initialisation pour y chercher du DRM.
const INIT_PROBE_BYTES: usize = 256 * 1024;

struct AuthState {
    access: String,
    expires_at: Instant,
    refresh: String,
    session: SessionInfo,
}

pub struct TidalClient {
    http: reqwest::Client,
    creds: ClientCredentials,
    auth: RwLock<Option<AuthState>>,
    restored: OnceCell<()>,
    /// Signalé quand TIDAL refuse le rafraîchissement en cours de route (#259).
    session_lost: Notify,
    pub cache: TrackCache,
    pub(super) hires: HiresCache,
    pub(super) likes: Likes,
    /// Identifiants de « Plus tard » (#196), mêmes ensembles que les favoris.
    pub(super) later: Likes,
    /// Titres et artistes bloqués (#334), pour filtrer mixes et radios.
    pub(super) blocks: super::blocks::Blocks,
    /// Titres lisibles ou non, retenus pour la session (#383).
    pub(super) availability: super::availability::Availability,
    pub(super) playlists: PlaylistCache,
    pub(super) versions: super::versions::PlaylistVersions,
    pub(super) home: super::home::HomeCache,
    /// Sorties récentes des artistes suivis (#415).
    pub(super) releases: super::releases::ReleasesCache,
    /// Pause shared by all requests after a 429 (#37).
    backoff: Backoff,
}

impl TidalClient {
    pub fn new() -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .https_only(true)
            .timeout(Duration::from_secs(15))
            .build()?;
        Ok(Self {
            http,
            creds: ClientCredentials::load()?,
            auth: RwLock::new(None),
            restored: OnceCell::new(),
            session_lost: Notify::new(),
            cache: TrackCache::default(),
            hires: HiresCache::default(),
            likes: Likes::default(),
            later: Likes::default(),
            blocks: Default::default(),
            availability: Default::default(),
            playlists: PlaylistCache::default(),
            versions: Default::default(),
            home: Default::default(),
            releases: Default::default(),
            backoff: Backoff::default(),
        })
    }

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

    /// Métadonnées d'un titre, depuis le cache si possible.
    pub async fn track(&self, id: TrackId) -> Result<Track> {
        if let Some(track) = self.cache.get(id) {
            return Ok(track);
        }
        let raw: RawTrack = self.get(&format!("tracks/{id}"), &[]).await?;
        let track = raw
            .into_track()
            .ok_or_else(|| TidalError::Decode(format!("titre {id} sans album")))?;
        self.cache.insert_all([&track]);
        Ok(track)
    }

    /// Demande un flux et refuse tout flux chiffré (manifeste ou segment d'initialisation).
    pub async fn stream(&self, id: TrackId, quality: AudioQuality) -> Result<Stream> {
        let (raw, decoded) = self.playback(id, quality).await?;
        if let Some(init_url) = &decoded.init_url {
            self.check_init_segment(init_url).await?;
        }
        Ok(Stream {
            format: stream_format(&raw, &decoded),
            source: decoded.source,
            gain: ReplayGain {
                track_gain: raw.track_replay_gain,
                track_peak: raw.track_peak_amplitude,
                album_gain: raw.album_replay_gain,
                album_peak: raw.album_peak_amplitude,
            },
        })
    }

    /// Format servi pour `id`, sans lire le flux : sert à proposer une édition hi-res (#59).
    pub async fn playback_format(
        &self,
        id: TrackId,
        quality: AudioQuality,
    ) -> Result<StreamFormat> {
        let (raw, decoded) = self.playback(id, quality).await?;
        Ok(stream_format(&raw, &decoded))
    }

    /// `playbackinfopostpaywall` et son manifeste décodé (refusé s'il déclare du DRM). Le
    /// segment d'initialisation n'est pas lu : `stream` le vérifie avant toute lecture.
    pub(super) async fn playback(
        &self,
        id: TrackId,
        quality: AudioQuality,
    ) -> Result<(RawPlayback, manifest::Decoded)> {
        let raw: RawPlayback = self
            .get(
                &format!("tracks/{id}/playbackinfopostpaywall"),
                &[
                    ("playbackmode", "STREAM".to_owned()),
                    ("audioquality", quality.as_api_str().to_owned()),
                    ("assetpresentation", "FULL".to_owned()),
                ],
            )
            .await?;
        let decoded = manifest::decode(&raw.manifest_mime_type, &raw.manifest)?;
        Ok((raw, decoded))
    }

    async fn check_init_segment(&self, url: &str) -> Result<()> {
        let mut resp = self.http.get(url).send().await?.error_for_status()?;
        let mut head = Vec::new();
        while let Some(chunk) = resp.chunk().await? {
            head.extend_from_slice(&chunk);
            if head.len() >= INIT_PROBE_BYTES {
                break;
            }
        }
        match manifest::find_init_drm_box(&head) {
            Some(b) => Err(TidalError::Drm(format!("boîte {b} dans le segment d'init"))),
            None => Ok(()),
        }
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
    async fn credentials(&self, rejected: Option<&str>) -> Result<(String, String)> {
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

    /// GET sur l'API v1 avec `countryCode`, en rafraîchissant le token sur 401.
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
            .map_err(|e| TidalError::Decode(format!("{path} : {e}")))
    }

    /// Requête authentifiée avec `countryCode`, en rafraîchissant le token sur 401.
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

    async fn send(&self, access: &str, call: &Call<'_>) -> Result<reqwest::Response> {
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
            // JSON:API de `openapi.tidal.com` (#196) : un autre type de contenu y est refusé.
            req = req
                .header(reqwest::header::CONTENT_TYPE, JSON_API)
                .body(json.to_string());
        }
        if let Some(etag) = call.if_none_match {
            req = req.header("If-None-Match", etag);
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
                format!("{path} introuvable ({message})")
            } else {
                message
            },
        })
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

/// Une requête vers l'API : GET par défaut, ou écriture avec formulaire et ETag.
#[derive(Clone)]
pub(super) struct Call<'a> {
    pub method: Method,
    pub base: &'a str,
    pub path: &'a str,
    pub query: &'a [(&'a str, String)],
    /// Corps `application/x-www-form-urlencoded`.
    pub form: Option<&'a [(&'a str, String)]>,
    /// Corps JSON:API (`application/vnd.api+json`), pour `openapi.tidal.com`.
    pub json: Option<&'a serde_json::Value>,
    /// ETag de la ressource : TIDAL refuse une modification de playlist sans lui.
    pub if_none_match: Option<&'a str>,
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
        }
    }
}

// --- Formats JSON bruts --------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSession {
    user_id: u64,
    country_code: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawApiError {
    user_message: Option<String>,
    /// Format des erreurs de l'API v2.
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

/// Réponse de `playbackinfopostpaywall` (vérifiée le 2026-10-30) : le gain et les crêtes
/// ReplayGain y sont aussi, lus par les infos du titre (#3).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawPlayback {
    audio_quality: AudioQuality,
    manifest_mime_type: String,
    manifest: String,
    bit_depth: Option<u8>,
    sample_rate: Option<u32>,
    pub track_replay_gain: Option<f64>,
    pub track_peak_amplitude: Option<f64>,
    pub album_replay_gain: Option<f64>,
    pub album_peak_amplitude: Option<f64>,
}

pub(super) fn stream_format(raw: &RawPlayback, decoded: &manifest::Decoded) -> StreamFormat {
    StreamFormat {
        quality: raw.audio_quality,
        codec: decoded.codec.clone(),
        bit_depth: raw.bit_depth,
        sample_rate: raw.sample_rate,
        bitrate: decoded.bitrate,
        hires_edition: None,
    }
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
