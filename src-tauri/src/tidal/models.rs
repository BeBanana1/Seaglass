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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistRef {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumRef {
    pub id: u64,
    pub title: String,
    /// URL d'une pochette 320×320, si disponible.
    pub cover_url: Option<String>,
    /// Couleur dominante de la pochette (`#rrggbb`), pour accorder l'interface.
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// Titre d'une liste (playlist ou favoris) avec sa date d'ajout à cette liste.
/// Sérialisé à plat : côté UI, c'est un `Track` avec un champ `addedAt` en plus.
/// La date n'appartient qu'à la liste : elle ne va ni dans le cache ni dans la file.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackEntry {
    #[serde(flatten)]
    pub track: Track,
    /// Millisecondes depuis l'epoch Unix (UTC) : `dateAdded` d'un titre de playlist,
    /// `created` d'un favori. `None` si l'API ne la donne pas ou si elle est illisible.
    pub added_at: Option<i64>,
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
pub struct AlbumSummary {
    pub id: u64,
    pub title: String,
    pub artists: Vec<ArtistRef>,
    pub cover_url: Option<String>,
    /// Couleur dominante de la pochette (`#rrggbb`).
    pub color: Option<String>,
    /// `AAAA-MM-JJ`.
    pub release_date: Option<String>,
    pub number_of_tracks: Option<u32>,
    /// `ALBUM`, `EP` ou `SINGLE`.
    pub album_type: Option<String>,
    pub explicit: bool,
    pub hires: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistSummary {
    pub id: u64,
    pub name: String,
    pub picture_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistSummary {
    pub uuid: String,
    pub title: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub number_of_tracks: Option<u32>,
    /// Durée totale en secondes.
    pub duration: Option<u32>,
    /// Playlist éditoriale du service.
    pub editorial: bool,
    /// Créée par l'utilisateur connecté.
    pub mine: bool,
}

/// Mix généré par le service (My Mix, Daily Discovery, mix d'artiste…). Son contenu
/// change dans le temps : il n'est jamais mis en cache.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MixSummary {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    /// URL d'une image 320×320.
    pub image_url: Option<String>,
    /// `DAILY_MIX`, `DISCOVERY_MIX`, `ARTIST_MIX`, `TRACK_MIX`…
    pub mix_type: Option<String>,
}

/// Élément affichable sous forme de carte (accueil, résultats).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MediaItem {
    Album(AlbumSummary),
    Artist(ArtistSummary),
    Playlist(PlaylistSummary),
    Mix(MixSummary),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeRow {
    /// Identifiant stable de la ligne (sert à la masquer ou la déplacer), voir `catalog::row_key`.
    pub key: String,
    pub title: String,
    pub items: Vec<MediaItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumPage {
    pub album: AlbumSummary,
    /// Pochette en grand format pour l'en-tête.
    pub cover_large_url: Option<String>,
    pub duration: Option<u32>,
    pub copyright: Option<String>,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistPage {
    pub artist: ArtistSummary,
    pub top_tracks: Vec<Track>,
    pub albums: Vec<AlbumSummary>,
    pub singles: Vec<AlbumSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistPage {
    pub playlist: PlaylistSummary,
    /// Dans l'ordre de la playlist.
    pub tracks: Vec<TrackEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MixPage {
    pub mix: MixSummary,
    /// Image 640×640 pour l'en-tête.
    pub image_large_url: Option<String>,
    pub tracks: Vec<Track>,
}

/// Point de départ d'une radio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RadioSeed {
    Track,
    Artist,
}

/// Radio d'un titre ou d'un artiste : une liste de titres proches, calculée par le service.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RadioPage {
    pub seed: RadioSeed,
    pub id: u64,
    /// Titre du morceau ou nom de l'artiste de départ.
    pub title: String,
    pub image_url: Option<String>,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub tracks: Vec<Track>,
    pub albums: Vec<AlbumSummary>,
    pub artists: Vec<ArtistSummary>,
    pub playlists: Vec<PlaylistSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub user_id: u64,
    pub country_code: String,
}
