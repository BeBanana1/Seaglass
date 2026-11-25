//! Domain types exposed to the rest of the app. The UI sees them through `src/lib/ipc.ts`;
//! those deriving `ts_rs::TS` are generated into `src/lib/bindings/` (#44).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export, export_to = "../../src/lib/bindings/")
)]
pub struct TrackId(#[cfg_attr(test, ts(type = "number"))] pub u64);

impl std::fmt::Display for TrackId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export, export_to = "../../src/lib/bindings/")
)]
pub struct ArtistRef {
    #[cfg_attr(test, ts(type = "number"))]
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", from = "StoredAlbumRef")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export, export_to = "../../src/lib/bindings/")
)]
pub struct AlbumRef {
    #[cfg_attr(test, ts(type = "number"))]
    pub id: u64,
    pub title: String,
    /// URL of a 320×320 cover, if available.
    pub cover_url: Option<String>,
    /// Same cover at 160×160, for list thumbnails (#89).
    pub cover_small_url: Option<String>,
    /// Same cover at 640×640, for "Now playing".
    pub cover_large_url: Option<String>,
    /// Dominant color of the cover (`#rrggbb`), to tune the interface.
    pub color: Option<String>,
}

/// `AlbumRef` read back (SQLite): tracks saved before 0.4.36 only have the 320 cover, the
/// other sizes are derived from it.
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
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export, export_to = "../../src/lib/bindings/")
)]
pub struct Track {
    pub id: TrackId,
    pub title: String,
    pub version: Option<String>,
    /// Duration in seconds.
    pub duration: u32,
    pub artists: Vec<ArtistRef>,
    pub album: AlbumRef,
    pub explicit: bool,
    /// AI-generated according to the service (`ai` field, #369). Missing from tracks saved
    /// before 0.5.29.
    #[serde(default)]
    pub ai: bool,
    /// Available in 24 bits (`HIRES_LOSSLESS` tag).
    pub hires: bool,
    /// Identifier of the recording, shared by all its editions (#18). Missing from tracks
    /// saved before 0.4.8.
    #[serde(default)]
    pub isrc: Option<String>,
}

/// Track of a list (playlist or favorites) with the date it was added to that list.
/// Serialized flat: on the UI side, it is a `Track` with an extra `addedAt` field.
/// The date only belongs to the list: it goes neither into the cache nor into the queue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackEntry {
    #[serde(flatten)]
    pub track: Track,
    /// Milliseconds since the Unix epoch (UTC): `dateAdded` of a playlist track, `created`
    /// of a favorite. `None` if the API does not give it or it cannot be read.
    pub added_at: Option<i64>,
}

/// A library category (tracks, albums, artists, playlists), newest first, possibly cut
/// at the loading cap (#91).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryList<T> {
    pub items: Vec<T>,
    /// Cut at the cap: only the newest are there, more follow.
    pub truncated: bool,
    /// Count announced by the API (`totalNumberOfItems`, unavailable items included), if
    /// known: it can exceed `items.len()` without the list being cut.
    pub total: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export, export_to = "../../src/lib/bindings/")
)]
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

/// Format actually served by TIDAL for a given playback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export, export_to = "../../src/lib/bindings/")
)]
pub struct StreamFormat {
    pub quality: AudioQuality,
    pub codec: String,
    pub bit_depth: Option<u8>,
    pub sample_rate: Option<u32>,
    /// Average bitrate declared by the manifest, in kbps (#3).
    pub bitrate: Option<u32>,
    /// Album of the edition actually played when the stream comes from another hi-res edition
    /// of the same recording (#18); `None` for the track shown.
    pub hires_edition: Option<EditionAlbum>,
}

/// Album of another hi-res edition: shown and openable from "Now playing" (#111), without
/// changing the current track (like, playlists and queue keep its id).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export, export_to = "../../src/lib/bindings/")
)]
pub struct EditionAlbum {
    #[cfg_attr(test, ts(type = "number"))]
    pub id: u64,
    pub title: String,
}

impl From<&Track> for EditionAlbum {
    fn from(track: &Track) -> Self {
        Self {
            id: track.album.id,
            title: track.album.title.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum StreamSource {
    /// MPEG-DASH manifest (XML) served to mpv over local HTTP.
    Dash(String),
    /// Direct URL (BTS manifest).
    Direct(String),
}

#[derive(Debug, Clone)]
pub struct Stream {
    pub format: StreamFormat,
    pub source: StreamSource,
    /// ReplayGain of the stream actually played (hi-res edition included), for volume
    /// normalization (#82).
    pub gain: ReplayGain,
    /// Streaming session the play log reports this stream under (#34).
    pub session: PlaybackSession,
}

/// What a stream is requested for (#34): played now, or preloaded for gapless (sent as
/// `x-tidal-prefetch`, as the official clients do).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamUse {
    Play,
    Preload,
}

/// Streaming session of one stream request (#34): its id goes out with
/// `playbackinfopostpaywall` and comes back in the play log, with what TIDAL served.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackSession {
    /// UUID created by the client for this request.
    pub id: String,
    /// Track requested from TIDAL (a hi-res edition when the stream comes from one).
    pub requested: TrackId,
    /// Track TIDAL actually served (`trackId` of the answer).
    pub served: TrackId,
    /// `assetPresentation` of the answer (`FULL`, `PREVIEW`).
    pub presentation: String,
    /// `audioMode` of the answer (`STEREO`…).
    pub audio_mode: Option<String>,
    pub quality: AudioQuality,
}

/// Another client of the account started playing (#35, Pushkin's
/// `PRIVILEGED_SESSION_NOTIFICATION`): TIDAL allows one stream at a time, this one pauses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackTaken {
    /// What the other client calls itself (`clientDisplayName`: "Android" for the official
    /// app), when it says.
    pub device: Option<String>,
}

/// Where a listen was played from, as the play log names it (#34). Other sources are
/// reported under the track's album.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogSource {
    Album(u64),
    Playlist(String),
    Mix(String),
    Artist(u64),
}

/// Pause, resume or seek during a listen (#34), position in seconds, time in Unix ms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogAction {
    /// `PLAYBACK_START` if true, `PLAYBACK_STOP` otherwise.
    pub start: bool,
    pub position: f64,
    pub timestamp: i64,
}

/// One listen of one stream, from the first sound heard to the last (#34): the official
/// clients' `playback_session` event.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayLog {
    pub session: PlaybackSession,
    pub source: LogSource,
    pub start_timestamp: i64,
    pub start_position: f64,
    pub end_timestamp: i64,
    pub end_position: f64,
    pub actions: Vec<LogAction>,
}

/// ReplayGain from `playbackinfopostpaywall`: gains in dB, peaks in full scale (1 = 0 dBFS).
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
    /// Same cover at 640×640, for the home banner (#89).
    pub cover_large_url: Option<String>,
    /// Dominant color of the cover (`#rrggbb`).
    pub color: Option<String>,
    /// `YYYY-MM-DD`.
    pub release_date: Option<String>,
    pub number_of_tracks: Option<u32>,
    /// `ALBUM`, `EP` or `SINGLE`.
    pub album_type: Option<String>,
    pub explicit: bool,
    /// AI-generated according to the service (`ai` field, #369).
    #[serde(default)]
    pub ai: bool,
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
    /// Total duration in seconds.
    pub duration: Option<u32>,
    /// Editorial playlist of the service.
    pub editorial: bool,
    /// Created by the signed-in user.
    pub mine: bool,
}

/// Playlist folder of the library (#331). A single level: no subfolders.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistFolder {
    pub id: String,
    pub name: String,
    /// Uuids of the playlists filed there (yours and the ones you follow).
    pub playlists: Vec<String>,
}

/// Mix generated by the service (My Mix, Daily Discovery, artist mix…). Its content changes
/// over time: it is never cached.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MixSummary {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    /// URL of a 320×320 image.
    pub image_url: Option<String>,
    /// `DAILY_MIX`, `DISCOVERY_MIX`, `ARTIST_MIX`, `TRACK_MIX`…
    pub mix_type: Option<String>,
}

/// "Save for later" item (#196): a track (with the date it was added) or a card.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LaterItem {
    Track(TrackEntry),
    Album(AlbumSummary),
    Artist(ArtistSummary),
    Playlist(PlaylistSummary),
}

/// Item shown as a card (home, results).
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
    /// Stable row identifier (used to hide or move it), see `catalog::row_key`.
    pub key: String,
    pub title: String,
    pub items: Vec<MediaItem>,
}

/// Feed item: release of a followed artist (album) or monthly mix (mix).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedItem {
    /// Release date, in UTC milliseconds.
    pub occurred_at: Option<i64>,
    /// Already seen in a TIDAL client (read state, never changed by the app).
    pub seen: bool,
    pub item: MediaItem,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumPage {
    pub album: AlbumSummary,
    /// Large cover for the header.
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

/// Bottom of the album page (#350), loaded after it: other editions of the same album
/// (deluxe, remaster…) and other albums of the artist, the album shown excluded.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumRelated {
    pub versions: Vec<AlbumSummary>,
    pub by_artist: Vec<AlbumSummary>,
}

/// Extras of the artist page (#205), loaded after it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistExtra {
    pub bio: Option<Bio>,
    pub similar: Vec<ArtistSummary>,
    /// Albums the artist appears on (compilations, collaborations).
    pub appears_on: Vec<AlbumSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bio {
    /// Provider of the text ("TiVo"), to credit.
    pub source: Option<String>,
    /// Paragraphs of segments.
    pub paragraphs: Vec<Vec<BioPart>>,
}

/// Biography segment: text, or an internal TIDAL link to an artist or an album.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BioPart {
    Text { text: String },
    Artist { text: String, id: u64 },
    Album { text: String, id: u64 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistPage {
    pub playlist: PlaylistSummary,
    /// In playlist order.
    pub tracks: Vec<TrackEntry>,
    /// ETag of the playlist when the page was read (#152): to give back to `playlist` to know
    /// whether it changed since.
    pub revision: Option<String>,
}

/// Change proposed by "Clean up" (#382). `index` = position in the list read
/// (0 = first track).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CleanupItem {
    /// Copy of a track already at `kept_index`: same track (`same_track`) or same song in
    /// another edition.
    #[serde(rename_all = "camelCase")]
    Duplicate {
        index: usize,
        track: Track,
        kept_index: usize,
        /// The track kept.
        kept: Box<Track>,
        same_track: bool,
    },
    /// Track removed from the catalog, with the same song in a playable edition if one is
    /// found.
    #[serde(rename_all = "camelCase")]
    Unavailable {
        index: usize,
        track: Track,
        replacement: Option<Box<Track>>,
    },
}

impl CleanupItem {
    /// What accepting it does: remove the duplicate or the unavailable track with no
    /// replacement, replace otherwise.
    pub fn op(&self) -> ListOp {
        match self {
            Self::Unavailable {
                index,
                track,
                replacement: Some(with),
            } => ListOp::Replace {
                index: *index,
                track_id: track.id,
                with: with.id,
            },
            Self::Duplicate { index, track, .. } | Self::Unavailable { index, track, .. } => {
                ListOp::Remove {
                    index: *index,
                    track_id: track.id,
                }
            }
        }
    }
}

/// Proposed change, with the operation that applies it (the UI sends it back as is).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupChange {
    #[serde(flatten)]
    pub item: CleanupItem,
    pub op: ListOp,
    /// Checked by default. Not an unavailable track without a replacement: removing it is final
    /// (the service refuses to add it back), and it may come back to the catalog.
    pub suggested: bool,
}

impl From<CleanupItem> for CleanupChange {
    fn from(item: CleanupItem) -> Self {
        Self {
            op: item.op(),
            suggested: !matches!(
                item,
                CleanupItem::Unavailable {
                    replacement: None,
                    ..
                }
            ),
            item,
        }
    }
}

/// What "Clean up" proposes for a list, in list order.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupReport {
    pub items: Vec<CleanupChange>,
    /// Tracks read.
    pub scanned: usize,
}

/// Change to a list (#382). For a playlist, `index` = position when the operation
/// applies; favorites ignore it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum ListOp {
    #[serde(rename_all = "camelCase")]
    Remove { index: usize, track_id: TrackId },
    #[serde(rename_all = "camelCase")]
    Insert { index: usize, track_id: TrackId },
    /// `track_id` gives its place to `with`.
    #[serde(rename_all = "camelCase")]
    Replace {
        index: usize,
        track_id: TrackId,
        with: TrackId,
    },
}

/// Result of a series of changes: stopped at the first error.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListOpsResult {
    pub applied: usize,
    /// Skipped operations: a track the service refuses to add back (removed from the catalog).
    pub skipped: usize,
    /// Operations that undo the applied ones, in the order to apply them.
    pub undo: Vec<ListOp>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MixPage {
    pub mix: MixSummary,
    /// 640×640 image for the header.
    pub image_large_url: Option<String>,
    pub tracks: Vec<Track>,
}

/// Starting point of a radio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RadioSeed {
    Track,
    Artist,
}

/// What can be added to favorites (#4, #81), mixes included (#204).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LikeTarget {
    Track { id: TrackId },
    Album { id: u64 },
    Artist { id: u64 },
    Playlist { uuid: String },
    Mix { id: String },
}

/// What can be blocked (#334): offered neither by mixes nor by radios.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BlockTarget {
    Track { id: TrackId },
    Artist { id: u64 },
}

/// Blocked tracks and artists, newest first.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Blocked {
    pub tracks: Vec<Track>,
    pub artists: Vec<ArtistSummary>,
}

/// All favorites, as identifiers (`likes` event). Each list is sorted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LikedIds {
    pub tracks: Vec<TrackId>,
    pub albums: Vec<u64>,
    pub artists: Vec<u64>,
    /// Followed playlists (and sometimes yours: `favorites/ids` mixes them).
    pub playlists: Vec<String>,
    /// Saved mixes (#204), read separately: `favorites/ids` does not hold them.
    pub mixes: Vec<String>,
}

/// Radio of a track or an artist: a list of close tracks, computed by the service.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RadioPage {
    pub seed: RadioSeed,
    pub id: u64,
    /// Title of the starting track or name of the starting artist.
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

/// Lyrics of a track (`tracks/{id}/lyrics`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lyrics {
    pub track_id: TrackId,
    /// Provider to credit ("MUSIXMATCH").
    pub provider: Option<String>,
    /// Right-to-left script.
    pub rtl: bool,
    pub text: LyricsText,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LyricsText {
    /// Timed lines, sorted by time; an empty line marks an instrumental passage.
    Synced { lines: Vec<LyricLine> },
    /// Text without timing, line by line (empty lines = break between verses).
    Plain { lines: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    /// Start of the line, in milliseconds.
    pub time_ms: u32,
    pub text: String,
}

/// Detailed info of a track (#3), asked for when the panel opens.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackInfo {
    pub track: Track,
    /// Availability tags (`LOSSLESS`, `HIRES_LOSSLESS`, `DOLBY_ATMOS`…).
    pub tags: Vec<String>,
    /// Format served at maximum quality, as for playback; `None` if refused.
    pub stream: Option<StreamFormat>,
    /// Why the stream was refused (DRM, track unavailable…).
    pub stream_error: Option<String>,
    /// ReplayGain (dB) and peaks (full scale = 1), read but never applied.
    pub track_gain: Option<f64>,
    pub track_peak: Option<f64>,
    pub album_gain: Option<f64>,
    pub album_peak: Option<f64>,
    pub bpm: Option<u32>,
    /// Key as given by the API (`B`, `FSharp`…) and its mode (`MAJOR`, `MINOR`).
    pub key: Option<String>,
    pub key_scale: Option<String>,
    pub copyright: Option<String>,
    pub track_number: Option<u32>,
    pub volume_number: Option<u32>,
    /// Other hi-res edition of the same recording (#18, #55), if the track is not hi-res.
    pub hires_edition: Option<HiresEdition>,
    /// Credits grouped by role (#84); empty if TIDAL has none.
    pub credits: Vec<CreditGroup>,
}

/// A role (`Composer`, `Producer`, `Bass guitar`…, free English text from the API) and its
/// contributors.
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
    /// Matching TIDAL artist (artist page), if there is one.
    pub artist_id: Option<u64>,
}

/// Credits of a whole album (#206): a role and its contributors, with the tracks each one
/// appears on.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumCreditGroup {
    pub role: String,
    pub contributors: Vec<AlbumContributor>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumContributor {
    pub name: String,
    pub artist_id: Option<u64>,
    /// Tracks they appear on, in album order; empty = every track. Identifiers rather than
    /// numbers: the UI places them in the list it shows.
    pub tracks: Vec<TrackId>,
}

/// Page of a contributor (#429): tracks a person is credited on, in batches.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContributorPage {
    pub artist: ArtistSummary,
    /// Number of tracks announced by the service, for the whole catalog.
    pub total: u32,
    pub tracks: Vec<Contribution>,
    /// To give back to `contributor` for the next batch; `None` at the end.
    pub next_offset: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Contribution {
    pub track: Track,
    /// Roles on this track (`Producer`, `Composer`…, free English text from the API).
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HiresEdition {
    pub track: Track,
    pub stream: Option<StreamFormat>,
    /// Seaglass plays it instead of the track (setting on and stream actually better).
    pub played: bool,
}
