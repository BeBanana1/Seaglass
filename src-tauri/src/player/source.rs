//! Source du contexte de lecture (#124) : d'où viennent les titres du contexte, pour le
//! lien « Joué depuis » de l'UI. Gardée avec la file, sans effet sur la lecture.

use serde::{Deserialize, Serialize};

use crate::tidal::models::RadioSeed;

/// Longueur maximale d'un titre ou d'une requête gardés (ils viennent de l'UI).
pub const MAX_LABEL_CHARS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PlaySource {
    Album {
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
        id: u64,
        title: String,
    },
    /// Titres populaires d'un artiste.
    Artist {
        id: u64,
        name: String,
    },
    LikedTracks,
    /// Liste « Plus tard » (#196).
    Later,
    Search {
        query: String,
    },
}

impl PlaySource {
    /// Libellé nettoyé (espaces retirés, tronqué) ; `None` si un champ est invalide.
    /// `is_uuid` vient des commandes, qui valident l'entrée.
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
            Self::LikedTracks => Self::LikedTracks,
            Self::Later => Self::Later,
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
            panic!("album refusé");
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
}
