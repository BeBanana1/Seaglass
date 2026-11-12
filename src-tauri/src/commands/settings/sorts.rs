//! Tri mémorisé par liste (#93) : chaque playlist, et Bibliothèque › Titres, garde le
//! dernier tri choisi. Logique pure : la liste est stockée dans `UiSettings::sorts`, la
//! plus récemment triée en tête. Le filtre n'est pas mémorisé.

use crate::i18n::tr;
use serde::{Deserialize, Serialize};

use super::pins::is_uuid;

/// Nombre maximal de listes mémorisées ; au-delà, la moins récemment triée est oubliée.
pub const MAX_SORTS: usize = 200;

/// Identifiant de Bibliothèque › Titres ; toute autre liste est une playlist (uuid).
const LIKED_TRACKS: &str = "likedTracks";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortKey {
    /// Ordre de la playlist.
    Position,
    /// Date d'ajout (ou du like).
    Added,
    Title,
    Artist,
    Album,
    Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackSort {
    pub key: SortKey,
    pub descending: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSort {
    /// Uuid de la playlist (en minuscules), ou `likedTracks`.
    pub list: String,
    #[serde(flatten)]
    pub sort: TrackSort,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SortError {
    #[error("{}", tr("identifiant de liste invalide", "invalid list ID"))]
    InvalidList,
}

/// Tri affiché quand rien n'est mémorisé : ordre de la playlist, ou like le plus récent
/// d'abord (ordre de l'API) pour les titres likés.
fn default_sort(list: &str) -> TrackSort {
    if list == LIKED_TRACKS {
        TrackSort {
            key: SortKey::Added,
            descending: true,
        }
    } else {
        TrackSort {
            key: SortKey::Position,
            descending: false,
        }
    }
}

/// Vrai si les tris venus d'une sauvegarde (#352) sont ceux que `update` aurait pu
/// enregistrer : listes valides, pas plus de `MAX_SORTS`.
pub(crate) fn are_valid(sorts: &[ListSort]) -> bool {
    sorts.len() <= MAX_SORTS
        && sorts
            .iter()
            .all(|s| s.list == LIKED_TRACKS || is_uuid(&s.list))
}

/// Mémorise le tri de `list` (en tête), ou l'oublie si `sort` est `None` ou le tri par
/// défaut : la liste ne garde que des valeurs utiles.
pub fn update(
    sorts: &mut Vec<ListSort>,
    list: &str,
    sort: Option<TrackSort>,
) -> Result<(), SortError> {
    let list = if list == LIKED_TRACKS {
        list.to_owned()
    } else if is_uuid(list) {
        list.to_ascii_lowercase()
    } else {
        return Err(SortError::InvalidList);
    };
    sorts.retain(|s| s.list != list);
    if let Some(sort) = sort.filter(|s| *s != default_sort(&list)) {
        sorts.insert(0, ListSort { list, sort });
        sorts.truncate(MAX_SORTS);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "1b7a5f3e-2c4d-4e6f-8a9b-0c1d2e3f4a5b";

    fn by(key: SortKey, descending: bool) -> TrackSort {
        TrackSort { key, descending }
    }

    #[test]
    fn serializes_flat() {
        let s = ListSort {
            list: UUID.into(),
            sort: by(SortKey::Title, true),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(
            json,
            format!(r#"{{"list":"{UUID}","key":"title","descending":true}}"#)
        );
        assert_eq!(serde_json::from_str::<ListSort>(&json).unwrap(), s);
        assert!(serde_json::from_str::<TrackSort>(r#"{"key":"bpm","descending":false}"#).is_err());
    }

    #[test]
    fn latest_first_and_replaced() {
        let other = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
        let mut sorts = Vec::new();
        update(&mut sorts, UUID, Some(by(SortKey::Title, false))).unwrap();
        update(&mut sorts, LIKED_TRACKS, Some(by(SortKey::Artist, false))).unwrap();
        update(&mut sorts, other, Some(by(SortKey::Duration, true))).unwrap();
        update(
            &mut sorts,
            &UUID.to_uppercase(),
            Some(by(SortKey::Album, true)),
        )
        .unwrap();
        let lists: Vec<&str> = sorts.iter().map(|s| s.list.as_str()).collect();
        assert_eq!(lists, [UUID, other, LIKED_TRACKS]);
        assert_eq!(sorts[0].sort, by(SortKey::Album, true));
    }

    #[test]
    fn default_or_none_forgets() {
        let mut sorts = Vec::new();
        update(&mut sorts, UUID, Some(by(SortKey::Title, false))).unwrap();
        update(&mut sorts, LIKED_TRACKS, Some(by(SortKey::Title, false))).unwrap();
        update(&mut sorts, UUID, Some(by(SortKey::Position, false))).unwrap();
        update(&mut sorts, LIKED_TRACKS, Some(by(SortKey::Added, true))).unwrap();
        assert!(sorts.is_empty());

        update(&mut sorts, UUID, Some(by(SortKey::Title, false))).unwrap();
        update(&mut sorts, UUID, None).unwrap();
        assert!(sorts.is_empty());

        // Le défaut d'une liste n'est pas celui de l'autre.
        update(&mut sorts, UUID, Some(by(SortKey::Added, true))).unwrap();
        update(&mut sorts, LIKED_TRACKS, Some(by(SortKey::Position, false))).unwrap();
        assert_eq!(sorts.len(), 2);
    }

    #[test]
    fn rejects_malformed_list() {
        let mut sorts = Vec::new();
        for bad in [
            "",
            "liked",
            "../../users/1",
            "1b7a5f3e-2c4d-4e6f-8a9b-0c1d2e3f4a5g",
        ] {
            assert_eq!(
                update(&mut sorts, bad, Some(by(SortKey::Title, false))),
                Err(SortError::InvalidList),
                "{bad}"
            );
        }
        assert!(sorts.is_empty());
    }

    #[test]
    fn oldest_dropped_beyond_limit() {
        let mut sorts = Vec::new();
        for i in 0..=MAX_SORTS {
            let uuid = format!("00000000-0000-0000-0000-{i:012}");
            update(&mut sorts, &uuid, Some(by(SortKey::Title, false))).unwrap();
        }
        assert_eq!(sorts.len(), MAX_SORTS);
        assert_eq!(
            sorts[0].list,
            format!("00000000-0000-0000-0000-{MAX_SORTS:012}")
        );
        assert!(!sorts.iter().any(|s| s.list.ends_with("-000000000000")));
    }
}
