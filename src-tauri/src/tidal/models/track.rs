//! Tracks and what they point to: ids, artist and album references, list entries.

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
                .and_then(|u| crate::tidal::raw::resized(u, size))
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
