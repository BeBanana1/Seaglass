//! Source of the playback context (#124): where the context's tracks come from, for the
//! UI's "Playing from" link. Kept with the queue, with no effect on playback.

use serde::{Deserialize, Serialize};

use crate::store::SmartList;
use crate::tidal::models::RadioSeed;

/// Maximum length of a kept title or query (they come from the UI).
pub const MAX_LABEL_CHARS: usize = 200;

/// Origin of a playback context (#124), passed to `playTracks`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export, export_to = "../../src/lib/bindings/")
)]
pub enum PlaySource {
    Album {
        #[cfg_attr(test, ts(type = "number"))]
        id: u64,
        title: String,
    },
    Playlist {
        uuid: String,
        title: String,
    },
    Mix {
        id: String,
        title: String,
    },
    Radio {
        seed: RadioSeed,
        #[cfg_attr(test, ts(type = "number"))]
        id: u64,
        title: String,
    },
    /// An artist's top tracks.
    Artist {
        #[cfg_attr(test, ts(type = "number"))]
        id: u64,
        name: String,
    },
    /// Tracks where a person is credited (#429).
    Contributor {
        #[cfg_attr(test, ts(type = "number"))]
        id: u64,
        name: String,
    },
    LikedTracks,
    /// The "Save for later" list (#196).
    Later,
    Search {
        query: String,
    },
    /// Playlist intelligente locale (#383).
    Smart {
        list: SmartList,
    },
    /// Playlist from another service, played without being saved (#328): `link` = the
    /// playlist's clean address, to reopen the import.
    Import {
        link: String,
        title: String,
    },
}

impl PlaySource {
    /// Cleaned label (spaces trimmed, truncated); `None` if a field is invalid.
    /// `is_uuid` comes from the commands, which validate the input.
    pub fn sanitize(self, is_uuid: impl Fn(&str) -> bool) -> Option<Self> {
        fn label(s: String) -> Option<String> {
            let s = s.trim();
            (!s.is_empty()).then(|| s.chars().take(MAX_LABEL_CHARS).collect())
        }
        Some(match self {
            Self::Album { id, title } => Self::Album {
                id,
                title: label(title)?,
            },
            Self::Playlist { uuid, title } if is_uuid(&uuid) => Self::Playlist {
                uuid,
                title: label(title)?,
            },
            Self::Playlist { .. } => return None,
            Self::Mix { id, title } => {
                let valid = !id.is_empty()
                    && id.len() <= 64
                    && id.chars().all(|c| c.is_ascii_alphanumeric());
                if !valid {
                    return None;
                }
                Self::Mix {
                    id,
                    title: label(title)?,
                }
            }
            Self::Radio { seed, id, title } => Self::Radio {
                seed,
                id,
                title: label(title)?,
            },
            Self::Artist { id, name } => Self::Artist {
                id,
                name: label(name)?,
            },
            Self::Contributor { id, name } => Self::Contributor {
                id,
                name: label(name)?,
            },
            Self::LikedTracks => Self::LikedTracks,
            Self::Later => Self::Later,
            Self::Smart { list } => Self::Smart { list },
            Self::Import { link, title } => {
                // Read again like a pasted link: only a recognized playlist address gets through.
                let link = crate::foreign::playlist_ref_offline(&link)?.url();
                Self::Import {
                    link,
                    title: label(title)?,
                }
            }
            Self::Search { query } => Self::Search {
                query: label(query)?,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn any_uuid(s: &str) -> bool {
        s.len() == 36
    }

    #[test]
    fn serializes_with_kind_tag() {
        let s = PlaySource::Radio {
            seed: RadioSeed::Artist,
            id: 7,
            title: "Daft Punk".into(),
        };
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"kind": "radio", "seed": "artist", "id": 7, "title": "Daft Punk"})
        );
        assert_eq!(serde_json::from_value::<PlaySource>(json).unwrap(), s);
        assert_eq!(
            serde_json::to_value(PlaySource::LikedTracks).unwrap(),
            serde_json::json!({"kind": "likedTracks"})
        );
        assert_eq!(
            serde_json::to_value(PlaySource::Later).unwrap(),
            serde_json::json!({"kind": "later"})
        );
    }

    #[test]
    fn sanitize_trims_and_truncates() {
        let s = PlaySource::Album {
            id: 1,
            title: format!("  {}  ", "a".repeat(300)),
        };
        let Some(PlaySource::Album { title, .. }) = s.sanitize(any_uuid) else {
            panic!("album refused");
        };
        assert_eq!(title.chars().count(), MAX_LABEL_CHARS);
    }

    #[test]
    fn sanitize_rejects_invalid_fields() {
        let empty = PlaySource::Search { query: "  ".into() };
        assert_eq!(empty.sanitize(any_uuid), None);
        let bad_uuid = PlaySource::Playlist {
            uuid: "../x".into(),
            title: "P".into(),
        };
        assert_eq!(bad_uuid.sanitize(any_uuid), None);
        let bad_mix = PlaySource::Mix {
            id: "a/b".into(),
            title: "M".into(),
        };
        assert_eq!(bad_mix.sanitize(any_uuid), None);
        assert_eq!(
            PlaySource::LikedTracks.sanitize(any_uuid),
            Some(PlaySource::LikedTracks)
        );
    }

    /// #328: only a recognized playlist address gets through, cleaned.
    #[test]
    fn sanitize_import_links() {
        let import = |link: &str| PlaySource::Import {
            link: link.into(),
            title: "Été".into(),
        };
        assert_eq!(
            import("https://www.deezer.com/fr/playlist/42?utm=x").sanitize(any_uuid),
            Some(import("https://www.deezer.com/playlist/42"))
        );
        assert_eq!(
            import("https://exemple.com/playlist/42").sanitize(any_uuid),
            None
        );
        assert_eq!(
            import("https://www.deezer.com/track/42").sanitize(any_uuid),
            None
        );
    }
}
