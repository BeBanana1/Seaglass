//! Client HTTP TIDAL : authentification, requêtes, titres et flux.
//! Les endpoints de catalogue (albums, artistes, playlists, favoris, accueil) sont dans
//! `catalog.rs` ; les formats JSON bruts dans `raw.rs`.
//!
//! Endpoints vérifiés avec le spike : `sessions`, `tracks/{id}`,
//! `tracks/{id}/playbackinfopostpaywall`.

use std::time::Duration;

use serde::Deserialize;
use tokio::sync::{Notify, OnceCell, RwLock};

use super::auth::ClientCredentials;
use super::backoff::Backoff;
use super::cache::TrackCache;
use super::hires::HiresCache;
use super::likes::Likes;
use super::manifest;
use super::models::{
    AudioQuality, PlaybackSession, ReplayGain, Stream, StreamFormat, StreamUse, Track, TrackId,
};
use super::playlist::PlaylistCache;
use super::raw::RawTrack;
pub(super) use super::request::Call;
use super::session::AuthState;
use super::{Result, TidalError};

pub(super) const API_V1: &str = "https://api.tidal.com/v1/";
pub(super) const API_V2: &str = "https://api.tidal.com/v2/";
/// API officielle (JSON:API), acceptée avec le token du client PKCE (vérifié le
/// 2026-11-05, #196).
pub(super) const OPEN_API: &str = "https://openapi.tidal.com/v2/";
const USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 12; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/91.0.4472.114 Safari/537.36";
/// Taille maximale lue dans un segment d'initialisation pour y chercher du DRM.
const INIT_PROBE_BYTES: usize = 256 * 1024;

pub struct TidalClient {
    pub(super) http: reqwest::Client,
    pub(super) creds: ClientCredentials,
    pub(super) auth: RwLock<Option<AuthState>>,
    pub(super) restored: OnceCell<()>,
    /// Signalé quand TIDAL refuse le rafraîchissement en cours de route (#259).
    pub(super) session_lost: Notify,
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
    pub(super) backoff: Backoff,
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
    /// Each request opens its own streaming session, reported by the play log once the
    /// stream is heard (#34).
    pub async fn stream(
        &self,
        id: TrackId,
        quality: AudioQuality,
        use_: StreamUse,
    ) -> Result<Stream> {
        let session_id = super::playlog::new_uuid();
        let fetch = Fetch {
            session_id: &session_id,
            prefetch: use_ == StreamUse::Preload,
        };
        let (raw, decoded) = self.playback(id, quality, Some(fetch)).await?;
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
            session: PlaybackSession {
                id: session_id,
                requested: id,
                served: raw.track_id.map_or(id, TrackId),
                presentation: raw.asset_presentation.unwrap_or_else(|| "FULL".to_owned()),
                audio_mode: raw.audio_mode,
                quality: raw.audio_quality,
            },
        })
    }

    /// Format servi pour `id`, sans lire le flux : sert à proposer une édition hi-res (#59).
    pub async fn playback_format(
        &self,
        id: TrackId,
        quality: AudioQuality,
    ) -> Result<StreamFormat> {
        let (raw, decoded) = self.playback(id, quality, None).await?;
        Ok(stream_format(&raw, &decoded))
    }

    /// `playbackinfopostpaywall` and its decoded manifest (refused if it declares DRM). The
    /// init segment is not read: `stream` checks it before any playback. `fetch` only for a
    /// stream that may be played (#34): without it, nothing is reported.
    pub(super) async fn playback(
        &self,
        id: TrackId,
        quality: AudioQuality,
        fetch: Option<Fetch<'_>>,
    ) -> Result<(RawPlayback, manifest::Decoded)> {
        let path = format!("tracks/{id}/playbackinfopostpaywall");
        let query = [
            ("playbackmode", "STREAM".to_owned()),
            ("audioquality", quality.as_api_str().to_owned()),
            ("assetpresentation", "FULL".to_owned()),
        ];
        let mut headers = Vec::new();
        if let Some(fetch) = fetch {
            headers.push(("x-tidal-streamingsessionid", fetch.session_id));
            if fetch.prefetch {
                headers.push(("x-tidal-prefetch", "true"));
            }
        }
        let raw: RawPlayback = self
            .call(&Call {
                headers: &headers,
                ..Call::get(API_V1, &path, &query)
            })
            .await?
            .json()
            .await
            .map_err(|e| TidalError::Decode(format!("{path} : {e}")))?;
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
}

/// Streaming session sent with a stream request that may be played (#34).
#[derive(Clone, Copy)]
pub(super) struct Fetch<'a> {
    pub session_id: &'a str,
    /// Preloaded for gapless, not played yet (`x-tidal-prefetch`).
    pub prefetch: bool,
}

// --- Formats JSON bruts --------------------------------------------------------------

/// Réponse de `playbackinfopostpaywall` (vérifiée le 2026-10-30) : le gain et les crêtes
/// ReplayGain y sont aussi, lus par les infos du titre (#3).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawPlayback {
    /// Track actually served, reported in the play log (#34).
    track_id: Option<u64>,
    asset_presentation: Option<String>,
    audio_mode: Option<String>,
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
