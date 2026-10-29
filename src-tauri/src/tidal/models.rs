//! Types métier exposés au reste de l'app (et dupliqués dans `src/lib/ipc.ts`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TrackId(pub u64);

impl std::fmt::Display for TrackId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistRef {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumRef {
    pub id: u64,
    pub title: String,
    /// URL d'une pochette 320×320, si disponible.
    pub cover_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: TrackId,
    pub title: String,
    pub version: Option<String>,
    /// Durée en secondes.
    pub duration: u32,
    pub artists: Vec<ArtistRef>,
    pub album: AlbumRef,
    pub explicit: bool,
    /// Disponible en 24 bits (tag `HIRES_LOSSLESS`).
    pub hires: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AudioQuality {
    Low,
    High,
    Lossless,
    HiResLossless,
}

impl AudioQuality {
    pub fn as_api_str(self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::High => "HIGH",
            Self::Lossless => "LOSSLESS",
            Self::HiResLossless => "HI_RES_LOSSLESS",
        }
    }
}

/// Format réellement servi par TIDAL pour une lecture donnée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamFormat {
    pub quality: AudioQuality,
    pub codec: String,
    pub bit_depth: Option<u8>,
    pub sample_rate: Option<u32>,
}

#[derive(Debug, Clone)]
pub enum StreamSource {
    /// Manifeste MPEG-DASH (XML) à servir à mpv en HTTP local.
    Dash(String),
    /// URL directe (manifeste BTS).
    Direct(String),
}

#[derive(Debug, Clone)]
pub struct Stream {
    pub format: StreamFormat,
    pub source: StreamSource,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub user_id: u64,
    pub country_code: String,
}
