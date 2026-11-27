//! Pinned shortcuts (#10): playlists, mixes, liked tracks, "Later" and smart
//! playlists (#383). Sidebar
//! on desktop, driving mode tiles on Android (#193). Pure logic: the list is
//! stored in `UiSettings::pins`, in display order.
//!
//! The title of a playlist or mix is kept with the pin to display it without a
//! request; the UI updates it when it opens the playlist.

use std::collections::HashSet;

use crate::i18n::{tr, trf};
use crate::store::SmartList;
use serde::{Deserialize, Serialize};

/// Maximum number of pins.
pub const MAX_PINS: usize = 50;
/// Maximum length of a kept title (in characters), beyond it is truncated.
pub const MAX_TITLE_CHARS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Pin {
    Playlist {
        uuid: String,
        title: String,
    },
    /// TIDAL mix or radio (#193).
    Mix {
        id: String,
        title: String,
    },
    /// Library › Tracks.
    LikedTracks,
    /// Library › Later (#244).
    Later,
    /// Local smart playlist (#383).
    Smart {
        list: SmartList,
    },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PinError {
    #[error("{}", trf!(
        "trop d'épingles ({}, maximum {MAX_PINS})",
        "too many pins ({}, maximum {MAX_PINS})",
        .0
    ))]
    TooMany(usize),
    #[error("{}", tr("identifiant de playlist invalide", "invalid playlist ID"))]
    InvalidUuid,
    #[error("{}", tr("identifiant de mix invalide", "invalid mix ID"))]
    InvalidMixId,
}

impl Pin {
    /// Identity of the pin: two pins with the same key are duplicates.
    pub(crate) fn key(&self) -> &str {
        match self {
            Pin::Playlist { uuid, .. } => uuid,
            Pin::Mix { id, .. } => id,
            Pin::LikedTracks => "liked-tracks",
            Pin::Later => "later",
            Pin::Smart { list } => list.key(),
        }
    }
}

/// Validates the list sent by the UI: well-formed uuids, cleaned titles, duplicates removed
/// (the first occurrence keeps its place).
pub fn normalize(pins: Vec<Pin>) -> Result<Vec<Pin>, PinError> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(pins.len());
    for pin in pins {
        let pin = match pin {
            Pin::Playlist { uuid, title } => {
                if !is_uuid(&uuid) {
                    return Err(PinError::InvalidUuid);
                }
                Pin::Playlist {
                    uuid: uuid.to_ascii_lowercase(),
                    title: clean_title(&title),
                }
            }
            Pin::Mix { id, title } => {
                if !super::super::catalog::is_mix_id(&id) {
                    return Err(PinError::InvalidMixId);
                }
                Pin::Mix {
                    id,
                    title: clean_title(&title),
                }
            }
            Pin::LikedTracks | Pin::Later | Pin::Smart { .. } => pin,
        };
        if seen.insert(pin.key().to_owned()) {
            out.push(pin);
        }
    }
    if out.len() > MAX_PINS {
        return Err(PinError::TooMany(out.len()));
    }
    Ok(out)
}

fn clean_title(title: &str) -> String {
    title.trim().chars().take(MAX_TITLE_CHARS).collect()
}

/// `8-4-4-4-12` form in hexadecimal, that of TIDAL playlist uuids. The uuid ends up
/// in an API URL: nothing else is accepted.
pub(crate) fn is_uuid(s: &str) -> bool {
    let groups: Vec<&str> = s.split('-').collect();
    groups.len() == 5
        && groups
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(g, len)| g.len() == len && g.chars().all(|c| c.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "1b7a5f3e-2c4d-4e6f-8a9b-0c1d2e3f4a5b";

    fn playlist(uuid: &str, title: &str) -> Pin {
        Pin::Playlist {
            uuid: uuid.into(),
            title: title.into(),
        }
    }

    #[test]
    fn serializes_with_kind_tag() {
        let json =
            serde_json::to_string(&vec![playlist(UUID, "Soir"), Pin::LikedTracks, Pin::Later])
                .unwrap();
        assert_eq!(
            json,
            format!(
                r#"[{{"kind":"playlist","uuid":"{UUID}","title":"Soir"}},{{"kind":"likedTracks"}},{{"kind":"later"}}]"#
            )
        );
        assert!(serde_json::from_str::<Pin>(r#"{"kind":"album","id":1}"#).is_err());
    }

    #[test]
    fn keeps_order_and_drops_duplicates() {
        let other = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
        let pins = normalize(vec![
            playlist(UUID, "A"),
            Pin::LikedTracks,
            playlist(other, "B"),
            playlist(&UUID.to_uppercase(), "A bis"),
            Pin::LikedTracks,
            Pin::Later,
            Pin::Later,
        ])
        .unwrap();
        assert_eq!(
            pins,
            [
                playlist(UUID, "A"),
                Pin::LikedTracks,
                playlist(other, "B"),
                Pin::Later
            ]
        );
    }

    #[test]
    fn mix_pins() {
        let mix = |id: &str| Pin::Mix {
            id: id.into(),
            title: " Mix du jour ".into(),
        };
        let json = serde_json::to_string(&mix("0abc")).unwrap();
        assert_eq!(
            json,
            r#"{"kind":"mix","id":"0abc","title":" Mix du jour "}"#
        );
        // Title cleaned, duplicate removed.
        let pins = normalize(vec![mix("0abc"), mix("0abc"), Pin::LikedTracks]).unwrap();
        assert_eq!(
            pins,
            [
                Pin::Mix {
                    id: "0abc".into(),
                    title: "Mix du jour".into()
                },
                Pin::LikedTracks
            ]
        );
        for bad in ["", "../users/1", "abc?x=1"] {
            assert_eq!(
                normalize(vec![mix(bad)]),
                Err(PinError::InvalidMixId),
                "{bad}"
            );
        }
    }

    #[test]
    fn rejects_malformed_uuid() {
        for bad in [
            "",
            "abc",
            "../../users/1",
            "1b7a5f3e-2c4d-4e6f-8a9b-0c1d2e3f4a5g",
        ] {
            assert_eq!(
                normalize(vec![playlist(bad, "x")]),
                Err(PinError::InvalidUuid),
                "{bad}"
            );
        }
    }

    #[test]
    fn trims_and_truncates_titles() {
        let long = "é".repeat(MAX_TITLE_CHARS + 10);
        let pins = normalize(vec![playlist(UUID, &format!("  {long}  "))]).unwrap();
        let Pin::Playlist { title, .. } = &pins[0] else {
            panic!("playlist pin expected");
        };
        assert_eq!(title.chars().count(), MAX_TITLE_CHARS);
    }

    #[test]
    fn rejects_too_many() {
        let pins: Vec<Pin> = (0..=MAX_PINS)
            .map(|i| playlist(&format!("00000000-0000-0000-0000-{i:012}"), "x"))
            .collect();
        assert_eq!(normalize(pins), Err(PinError::TooMany(MAX_PINS + 1)));
    }
}
