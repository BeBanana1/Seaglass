//! Client HTTP TIDAL (API v1). Les formats JSON bruts restent privés à ce fichier.
//!
//! Endpoints vérifiés avec le spike : `sessions`, `search`, `tracks/{id}`,
//! `tracks/{id}/playbackinfopostpaywall`.

use std::time::{Duration, Instant};

use reqwest::{StatusCode, Url};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use tokio::sync::{OnceCell, RwLock};

use super::auth::{self, ClientCredentials, PkceChallenge, Tokens};
use super::cache::TrackCache;
use super::manifest;
use super::models::{
    AlbumRef, ArtistRef, AudioQuality, SessionInfo, Stream, StreamFormat, Track, TrackId,
};
use super::{Result, TidalError};

const API_V1: &str = "https://api.tidal.com/v1/";
const IMAGE_URL: &str = "https://resources.tidal.com/images";
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
    pub cache: TrackCache,
}

impl TidalClient {
    pub fn new() -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(15))
            .build()?;
        Ok(Self {
            http,
            creds: ClientCredentials::load()?,
            auth: RwLock::new(None),
            restored: OnceCell::new(),
            cache: TrackCache::default(),
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
            Ok(tokens) => self.install(tokens, refresh).await.map(drop),
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
        auth::clear_refresh_token()
    }

    pub async fn session(&self) -> Option<SessionInfo> {
        self.auth.read().await.as_ref().map(|a| a.session.clone())
    }

    pub async fn search_tracks(&self, query: &str, limit: u32) -> Result<Vec<Track>> {
        let raw: RawSearch = self
            .get(
                "search",
                &[
                    ("query", query.to_owned()),
                    ("types", "TRACKS".to_owned()),
                    ("limit", limit.to_string()),
                ],
            )
            .await?;
        let tracks: Vec<Track> = raw.tracks.items.into_iter().map(Track::from).collect();
        self.cache.insert_all(&tracks);
        Ok(tracks)
    }

    /// Métadonnées d'un titre, depuis le cache si possible.
    pub async fn track(&self, id: TrackId) -> Result<Track> {
        if let Some(track) = self.cache.get(id) {
            return Ok(track);
        }
        let raw: RawTrack = self.get(&format!("tracks/{id}"), &[]).await?;
        let track = Track::from(raw);
        self.cache.insert_all([&track]);
        Ok(track)
    }

    /// Demande un flux et refuse tout flux chiffré (manifeste ou segment d'initialisation).
    pub async fn stream(&self, id: TrackId, quality: AudioQuality) -> Result<Stream> {
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
        if let Some(init_url) = &decoded.init_url {
            self.check_init_segment(init_url).await?;
        }
        Ok(Stream {
            format: StreamFormat {
                quality: raw.audio_quality,
                codec: decoded.codec,
                bit_depth: raw.bit_depth,
                sample_rate: raw.sample_rate,
            },
            source: decoded.source,
        })
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
            .send(&tokens.access, "sessions", &[])
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

    /// Access token valide et code pays, en rafraîchissant si besoin (ou si `force`).
    async fn credentials(&self, force: bool) -> Result<(String, String)> {
        {
            let guard = self.auth.read().await;
            let state = guard.as_ref().ok_or(TidalError::NotLoggedIn)?;
            if !force && state.expires_at > Instant::now() + REFRESH_MARGIN {
                return Ok((state.access.clone(), state.session.country_code.clone()));
            }
        }
        let mut guard = self.auth.write().await;
        let state = guard.as_mut().ok_or(TidalError::NotLoggedIn)?;
        let tokens = auth::refresh(&self.http, &self.creds, &state.refresh).await?;
        state.access = tokens.access;
        state.expires_at = tokens.expires_at;
        Ok((state.access.clone(), state.session.country_code.clone()))
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, params: &[(&str, String)]) -> Result<T> {
        let mut force_refresh = false;
        loop {
            let (access, country) = self.credentials(force_refresh).await?;
            let mut query: Vec<(&str, String)> = vec![("countryCode", country)];
            query.extend(params.iter().cloned());
            let resp = self.send(&access, path, &query).await;
            match resp {
                Err(TidalError::Api { status: 401, .. }) if !force_refresh => {
                    force_refresh = true;
                }
                other => {
                    return other?
                        .json()
                        .await
                        .map_err(|e| TidalError::Decode(format!("{path} : {e}")));
                }
            }
        }
    }

    async fn send(
        &self,
        access: &str,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<reqwest::Response> {
        let resp = self
            .http
            .get(format!("{API_V1}{path}"))
            .query(query)
            .bearer_auth(access)
            .header("x-tidal-client-version", CLIENT_VERSION)
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let message = resp
            .json::<RawApiError>()
            .await
            .ok()
            .and_then(|e| e.user_message)
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
}

#[derive(Deserialize)]
struct RawSearch {
    tracks: RawPage<RawTrack>,
}

#[derive(Deserialize)]
struct RawPage<T> {
    items: Vec<T>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTrack {
    id: u64,
    title: String,
    version: Option<String>,
    duration: u32,
    #[serde(default)]
    explicit: bool,
    artists: Vec<RawArtist>,
    album: RawAlbum,
    media_metadata: Option<RawMediaMetadata>,
}

#[derive(Deserialize)]
struct RawArtist {
    id: u64,
    name: String,
}

#[derive(Deserialize)]
struct RawAlbum {
    id: u64,
    title: String,
    cover: Option<String>,
}

#[derive(Deserialize)]
struct RawMediaMetadata {
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPlayback {
    audio_quality: AudioQuality,
    manifest_mime_type: String,
    manifest: String,
    bit_depth: Option<u8>,
    sample_rate: Option<u32>,
}

impl From<RawTrack> for Track {
    fn from(raw: RawTrack) -> Self {
        let hires = raw
            .media_metadata
            .is_some_and(|m| m.tags.iter().any(|t| t == "HIRES_LOSSLESS"));
        Track {
            id: TrackId(raw.id),
            title: raw.title,
            version: raw.version,
            duration: raw.duration,
            artists: raw
                .artists
                .into_iter()
                .map(|a| ArtistRef {
                    id: a.id,
                    name: a.name,
                })
                .collect(),
            album: AlbumRef {
                id: raw.album.id,
                title: raw.album.title,
                cover_url: raw
                    .album
                    .cover
                    .map(|c| format!("{IMAGE_URL}/{}/320x320.jpg", c.replace('-', "/"))),
            },
            explicit: raw.explicit,
            hires,
        }
    }
}
