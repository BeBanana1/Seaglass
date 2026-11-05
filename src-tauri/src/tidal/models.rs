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
#[serde(rename_all = "camelCase", from = "StoredAlbumRef")]
pub struct AlbumRef {
    pub id: u64,
    pub title: String,
    /// URL d'une pochette 320×320, si disponible.
    pub cover_url: Option<String>,
    /// Même pochette en 160×160, pour les vignettes des listes (#89).
    pub cover_small_url: Option<String>,
    /// Même pochette en 640×640, pour « En lecture ».
    pub cover_large_url: Option<String>,
    /// Couleur dominante de la pochette (`#rrggbb`), pour accorder l'interface.
    pub color: Option<String>,
}

/// `AlbumRef` relu (SQLite) : les titres enregistrés avant 0.4.36 n'ont que la pochette 320,
/// les autres tailles en sont déduites.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredAlbumRef {
    id: u64,
    title: String,
    cover_url: Option<String>,
    #[serde(default)]
    cover_small_url: Option<String>,
    #[serde(default)]
    cover_large_url: Option<String>,
    #[serde(default)]
    color: Option<String>,
}

impl From<StoredAlbumRef> for AlbumRef {
    fn from(s: StoredAlbumRef) -> Self {
        let resized = |size| {
            s.cover_url
                .as_deref()
                .and_then(|u| super::raw::resized(u, size))
        };
        AlbumRef {
            cover_small_url: s.cover_small_url.or_else(|| resized(160)),
            cover_large_url: s.cover_large_url.or_else(|| resized(640)),
            id: s.id,
            title: s.title,
            cover_url: s.cover_url,
            color: s.color,
        }
    }
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
    /// Identifiant de l'enregistrement, commun à toutes ses éditions (#18). Absent des
    /// titres enregistrés avant la 0.4.8.
    #[serde(default)]
    pub isrc: Option<String>,
}

/// Titre d'une liste (playlist ou favoris) avec sa date d'ajout à cette liste.
/// Sérialisé à plat : côté UI, c'est un `Track` avec un champ `addedAt` en plus.
/// La date n'appartient qu'à la liste : elle ne va ni dans le cache ni dans la file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackEntry {
    #[serde(flatten)]
    pub track: Track,
    /// Millisecondes depuis l'epoch Unix (UTC) : `dateAdded` d'un titre de playlist,
    /// `created` d'un favori. `None` si l'API ne la donne pas ou si elle est illisible.
    pub added_at: Option<i64>,
}

/// Une catégorie de la bibliothèque (titres, albums, artistes, playlists), du plus récent au
/// plus ancien, éventuellement coupée au plafond de chargement (#91).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryList<T> {
    pub items: Vec<T>,
    /// Coupée au plafond : seuls les plus récents sont là, d'autres suivent.
    pub truncated: bool,
    /// Nombre annoncé par l'API (`totalNumberOfItems`, éléments indisponibles compris),
    /// s'il est connu : il peut dépasser `items.len()` sans que la liste soit coupée.
    pub total: Option<u32>,
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
    /// Débit moyen déclaré par le manifeste, en kbps (#3).
    pub bitrate: Option<u32>,
    /// Album de l'édition réellement lue quand le flux vient d'une autre édition hi-res du
    /// même enregistrement (#18) ; `None` pour le titre affiché.
    pub hires_edition: Option<String>,
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
    /// ReplayGain du flux réellement lu (édition hi-res comprise), pour la normalisation
    /// du volume (#82).
    pub gain: ReplayGain,
}

/// ReplayGain de `playbackinfopostpaywall` : gains en dB, crêtes en pleine échelle (1 = 0 dBFS).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ReplayGain {
    pub track_gain: Option<f64>,
    pub track_peak: Option<f64>,
    pub album_gain: Option<f64>,
    pub album_peak: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumSummary {
    pub id: u64,
    pub title: String,
    pub artists: Vec<ArtistRef>,
    pub cover_url: Option<String>,
    /// Même pochette en 640×640, pour le bandeau de l'accueil (#89).
    pub cover_large_url: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistSummary {
    pub id: u64,
    pub name: String,
    pub picture_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// Élément du fil d'actualité : sortie d'un artiste suivi (album) ou mix mensuel (mix).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedItem {
    /// Date de la sortie, en millisecondes UTC.
    pub occurred_at: Option<i64>,
    /// Déjà vue dans un client TIDAL (état lu, jamais modifié par l'app).
    pub seen: bool,
    pub item: MediaItem,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistPage {
    pub playlist: PlaylistSummary,
    /// Dans l'ordre de la playlist.
    pub tracks: Vec<TrackEntry>,
    /// ETag de la playlist quand la page a été lue (#152) : à rendre à `playlist` pour
    /// savoir si elle a changé depuis.
    pub revision: Option<String>,
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

/// Ce qu'on peut ajouter aux favoris (#4, #81). Pas de mix : les enregistrer n'est pas
/// encore proposé.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LikeTarget {
    Track { id: TrackId },
    Album { id: u64 },
    Artist { id: u64 },
    Playlist { uuid: String },
}

/// Tous les favoris, en identifiants (événement `likes`). Chaque liste est triée.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LikedIds {
    pub tracks: Vec<TrackId>,
    pub albums: Vec<u64>,
    pub artists: Vec<u64>,
    /// Playlists suivies (et parfois les vôtres : `favorites/ids` en mêle).
    pub playlists: Vec<String>,
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

/// Paroles d'un titre (`tracks/{id}/lyrics`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lyrics {
    pub track_id: TrackId,
    /// Fournisseur à créditer (« MUSIXMATCH »).
    pub provider: Option<String>,
    /// Écriture de droite à gauche.
    pub rtl: bool,
    pub text: LyricsText,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LyricsText {
    /// Lignes horodatées, triées par temps ; une ligne vide marque un passage instrumental.
    Synced { lines: Vec<LyricLine> },
    /// Texte sans horodatage, ligne par ligne (lignes vides = séparation de couplets).
    Plain { lines: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    /// Début de la ligne, en millisecondes.
    pub time_ms: u32,
    pub text: String,
}

/// Infos détaillées d'un titre (#3), demandées à l'ouverture du panneau.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackInfo {
    pub track: Track,
    /// Tags de disponibilité (`LOSSLESS`, `HIRES_LOSSLESS`, `DOLBY_ATMOS`…).
    pub tags: Vec<String>,
    /// Format servi en qualité maximale, comme pour une lecture ; `None` si refusé.
    pub stream: Option<StreamFormat>,
    /// Raison du refus du flux (DRM, titre indisponible…).
    pub stream_error: Option<String>,
    /// ReplayGain (dB) et crêtes (pleine échelle = 1), lus mais jamais appliqués.
    pub track_gain: Option<f64>,
    pub track_peak: Option<f64>,
    pub album_gain: Option<f64>,
    pub album_peak: Option<f64>,
    pub bpm: Option<u32>,
    /// Tonalité telle que donnée par l'API (`B`, `FSharp`…) et son mode (`MAJOR`, `MINOR`).
    pub key: Option<String>,
    pub key_scale: Option<String>,
    pub copyright: Option<String>,
    pub track_number: Option<u32>,
    pub volume_number: Option<u32>,
    /// Autre édition hi-res du même enregistrement (#18, #55), si le titre n'est pas hi-res.
    pub hires_edition: Option<HiresEdition>,
    /// Crédits groupés par rôle (#84) ; vide si TIDAL n'en a pas.
    pub credits: Vec<CreditGroup>,
}

/// Un rôle (`Composer`, `Producer`, `Bass guitar`…, texte libre en anglais de l'API) et
/// ses contributeurs.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditGroup {
    pub role: String,
    pub contributors: Vec<Contributor>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Contributor {
    pub name: String,
    /// Artiste TIDAL correspondant (page artiste), s'il y en a un.
    pub artist_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HiresEdition {
    pub track: Track,
    pub stream: Option<StreamFormat>,
    /// Seaglass la lit à la place du titre (réglage actif et flux réellement meilleur).
    pub played: bool,
}
