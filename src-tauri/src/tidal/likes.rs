//! Titres likés, gardés en mémoire pour afficher les cœurs sans requête (#4).
//!
//! Chargés d'un coup par `users/{id}/favorites/ids` (identifiants seuls, toute la
//! bibliothèque), puis tenus à jour par les likes faits dans l'app et par chaque lecture
//! complète des favoris. Un like fait sur un autre appareil n'apparaît qu'à ce moment-là.

use std::collections::HashSet;
use std::sync::Mutex;

use super::models::TrackId;

#[derive(Default)]
pub struct LikedTracks {
    ids: Mutex<Option<HashSet<TrackId>>>,
}

impl LikedTracks {
    /// `None` tant que rien n'a été chargé.
    pub fn get(&self) -> Option<Vec<TrackId>> {
        let guard = self.ids.lock().ok()?;
        guard.as_ref().map(sorted)
    }

    /// Remplace l'ensemble ; renvoie `true` s'il a changé.
    pub fn replace(&self, ids: impl IntoIterator<Item = TrackId>) -> bool {
        let Ok(mut guard) = self.ids.lock() else {
            return false;
        };
        let ids: HashSet<TrackId> = ids.into_iter().collect();
        let changed = guard.as_ref() != Some(&ids);
        *guard = Some(ids);
        changed
    }

    /// Applique un like ou son retrait, si l'ensemble est chargé.
    pub fn set(&self, id: TrackId, liked: bool) {
        if let Ok(mut guard) = self.ids.lock()
            && let Some(ids) = guard.as_mut()
        {
            if liked {
                ids.insert(id);
            } else {
                ids.remove(&id);
            }
        }
    }

    pub fn clear(&self) {
        if let Ok(mut guard) = self.ids.lock() {
            *guard = None;
        }
    }
}

/// Ordre stable : l'UI reçoit la même liste pour le même ensemble.
fn sorted(ids: &HashSet<TrackId>) -> Vec<TrackId> {
    let mut out: Vec<TrackId> = ids.iter().copied().collect();
    out.sort_by_key(|id| id.0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_before_load_is_ignored() {
        let likes = LikedTracks::default();
        likes.set(TrackId(1), true);
        assert_eq!(likes.get(), None);
    }

    #[test]
    fn replace_reports_changes() {
        let likes = LikedTracks::default();
        assert!(likes.replace([TrackId(2), TrackId(1)]));
        assert!(!likes.replace([TrackId(1), TrackId(2)]));
        assert_eq!(likes.get(), Some(vec![TrackId(1), TrackId(2)]));
        likes.set(TrackId(3), true);
        likes.set(TrackId(1), false);
        assert_eq!(likes.get(), Some(vec![TrackId(2), TrackId(3)]));
        likes.clear();
        assert_eq!(likes.get(), None);
    }
}
