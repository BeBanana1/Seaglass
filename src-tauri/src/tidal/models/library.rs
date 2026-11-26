//! Library writes: likes, blocks, "Clean up" and list changes.

use serde::{Deserialize, Serialize};

use super::{ArtistSummary, Track, TrackId};

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
