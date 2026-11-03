//! Raccourcis épinglés dans la barre latérale (#10) : playlists et titres likés.
//! Logique pure : la liste est stockée dans `UiSettings::pins`, dans l'ordre d'affichage.
//!
//! Le titre d'une playlist est gardé avec l'épingle pour afficher la barre sans requête ;
//! l'UI le remet à jour quand elle ouvre la playlist.

use std::collections::HashSet;

use crate::i18n::{tr, trf};
use serde::{Deserialize, Serialize};

/// Nombre maximal d'épingles.
pub const MAX_PINS: usize = 50;
/// Longueur maximale d'un titre gardé (en caractères), au-delà il est tronqué.
pub const MAX_TITLE_CHARS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Pin {
    Playlist {
        uuid: String,
        title: String,
    },
    /// Bibliothèque › Titres.
    LikedTracks,
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
}

impl Pin {
    /// Identité de l'épingle : deux épingles de même clé sont des doublons.
    fn key(&self) -> &str {
        match self {
            Pin::Playlist { uuid, .. } => uuid,
            Pin::LikedTracks => "liked-tracks",
        }
    }
}

/// Valide la liste envoyée par l'UI : uuid bien formés, titres nettoyés, doublons retirés
/// (la première occurrence garde sa place).
pub fn normalize(pins: Vec<Pin>) -> Result<Vec<Pin>, PinError> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(pins.len());
    for pin in pins {
        let pin = match pin {
            Pin::Playlist { uuid, title } => {
                if !is_uuid(&uuid) {
                    return Err(PinError::InvalidUuid);
                }
                let title = title.trim().chars().take(MAX_TITLE_CHARS).collect();
                Pin::Playlist {
                    uuid: uuid.to_ascii_lowercase(),
                    title,
                }
            }
            Pin::LikedTracks => Pin::LikedTracks,
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

/// Forme `8-4-4-4-12` en hexadécimal, celle des uuid de playlists TIDAL. L'uuid finit
/// dans une URL d'API : rien d'autre n'est accepté.
pub(super) fn is_uuid(s: &str) -> bool {
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
        let json = serde_json::to_string(&vec![playlist(UUID, "Soir"), Pin::LikedTracks]).unwrap();
        assert_eq!(
            json,
            format!(
                r#"[{{"kind":"playlist","uuid":"{UUID}","title":"Soir"}},{{"kind":"likedTracks"}}]"#
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
        ])
        .unwrap();
        assert_eq!(
            pins,
            [playlist(UUID, "A"), Pin::LikedTracks, playlist(other, "B")]
        );
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
            panic!("épingle de playlist attendue");
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
