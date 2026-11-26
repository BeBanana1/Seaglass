//! Queue track with its origin: current track, history and manual queue.

use serde::{Deserialize, Serialize};

use crate::tidal::models::TrackId;

/// Where a track comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Origin {
    /// From the current context: the `pos` cursor points to it.
    Context,
    /// Added by the user to the manual queue ("Play next", "Add to queue").
    Manual,
    /// Neither: track of a context replaced since, or current track after "Clear queue".
    Detached,
}

/// In the manual queue, an origin other than `Manual` marks a track left through
/// "Previous", put back at the front so that "Next" returns to it: a new context discards it
/// (#166: these tracks piled up and came before any new list).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "StoredEntry")]
pub(super) struct Entry {
    pub id: TrackId,
    pub origin: Origin,
}

impl Entry {
    pub fn new(id: TrackId, origin: Origin) -> Self {
        Self { id, origin }
    }

    pub fn in_context(self) -> bool {
        self.origin == Origin::Context
    }

    /// Added by the user, as opposed to a track recalled by "Previous".
    pub fn queued_by_user(self) -> bool {
        self.origin == Origin::Manual
    }
}

/// `Entry` as saved. Before 0.4.64: a plain id in the manual queue, and
/// `{id, from_context}` elsewhere (false = manual queue, without distinguishing a detached
/// track).
#[derive(Deserialize)]
#[serde(untagged)]
enum StoredEntry {
    Id(TrackId),
    Full {
        id: TrackId,
        origin: Option<Origin>,
        #[serde(default)]
        from_context: bool,
    },
}

impl From<StoredEntry> for Entry {
    fn from(s: StoredEntry) -> Self {
        match s {
            StoredEntry::Id(id) => Self::new(id, Origin::Manual),
            StoredEntry::Full {
                id,
                origin,
                from_context,
            } => {
                let legacy = if from_context {
                    Origin::Context
                } else {
                    Origin::Manual
                };
                Self::new(id, origin.unwrap_or(legacy))
            }
        }
    }
}
