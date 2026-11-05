//! Favoris gardés en mémoire pour afficher cœurs et boutons « Suivre » sans requête :
//! titres (#4), albums, artistes et playlists suivies (#81), mixes (#204).
//!
//! Chargés d'un coup par `users/{id}/favorites/ids` (identifiants seuls, toute la
//! bibliothèque), puis tenus à jour par les écritures faites dans l'app et par chaque
//! lecture complète d'une catégorie de la bibliothèque. Un favori ajouté sur un autre
//! appareil n'apparaît qu'à ce moment-là.

use std::collections::HashSet;
use std::hash::Hash;
use std::sync::Mutex;

use super::models::{LikeTarget, LikedIds, TrackId};

/// Catégorie remplacée par une lecture complète de la bibliothèque.
pub enum LikedKind {
    Tracks(Vec<TrackId>),
    Albums(Vec<u64>),
    Artists(Vec<u64>),
    Playlists(Vec<String>),
    Mixes(Vec<String>),
}

#[derive(Default, PartialEq)]
struct Sets {
    tracks: HashSet<TrackId>,
    albums: HashSet<u64>,
    artists: HashSet<u64>,
    playlists: HashSet<String>,
    mixes: HashSet<String>,
}

#[derive(Default)]
pub struct Likes {
    sets: Mutex<Option<Sets>>,
}

impl Likes {
    /// `None` tant que rien n'a été chargé.
    pub fn get(&self) -> Option<LikedIds> {
        let guard = self.sets.lock().ok()?;
        guard.as_ref().map(|s| LikedIds {
            tracks: sorted(&s.tracks, |id| id.0),
            albums: sorted(&s.albums, |id| *id),
            artists: sorted(&s.artists, |id| *id),
            playlists: sorted(&s.playlists, Clone::clone),
            mixes: sorted(&s.mixes, Clone::clone),
        })
    }

    /// Remplace tout (lecture de `favorites/ids`).
    pub fn load(&self, ids: LikedIds) {
        if let Ok(mut guard) = self.sets.lock() {
            *guard = Some(Sets {
                tracks: ids.tracks.into_iter().collect(),
                albums: ids.albums.into_iter().collect(),
                artists: ids.artists.into_iter().collect(),
                playlists: ids.playlists.into_iter().collect(),
                mixes: ids.mixes.into_iter().collect(),
            });
        }
    }

    /// Remplace une catégorie, si l'ensemble est chargé ; renvoie `true` si elle a changé.
    pub fn replace(&self, kind: LikedKind) -> bool {
        let Ok(mut guard) = self.sets.lock() else {
            return false;
        };
        let Some(sets) = guard.as_mut() else {
            return false;
        };
        match kind {
            LikedKind::Tracks(ids) => swap(&mut sets.tracks, ids),
            LikedKind::Albums(ids) => swap(&mut sets.albums, ids),
            LikedKind::Artists(ids) => swap(&mut sets.artists, ids),
            LikedKind::Playlists(ids) => swap(&mut sets.playlists, ids),
            LikedKind::Mixes(ids) => swap(&mut sets.mixes, ids),
        }
    }

    /// Applique un ajout ou un retrait, si l'ensemble est chargé.
    pub fn set(&self, target: &LikeTarget, liked: bool) {
        let Ok(mut guard) = self.sets.lock() else {
            return;
        };
        let Some(sets) = guard.as_mut() else {
            return;
        };
        match target {
            LikeTarget::Track { id } => toggle(&mut sets.tracks, *id, liked),
            LikeTarget::Album { id } => toggle(&mut sets.albums, *id, liked),
            LikeTarget::Artist { id } => toggle(&mut sets.artists, *id, liked),
            LikeTarget::Playlist { uuid } => toggle(&mut sets.playlists, uuid.clone(), liked),
            LikeTarget::Mix { id } => toggle(&mut sets.mixes, id.clone(), liked),
        }
    }

    pub fn clear(&self) {
        if let Ok(mut guard) = self.sets.lock() {
            *guard = None;
        }
    }
}

fn swap<T: Eq + Hash>(set: &mut HashSet<T>, ids: Vec<T>) -> bool {
    let ids: HashSet<T> = ids.into_iter().collect();
    let changed = *set != ids;
    *set = ids;
    changed
}

fn toggle<T: Eq + Hash>(set: &mut HashSet<T>, id: T, liked: bool) {
    if liked {
        set.insert(id);
    } else {
        set.remove(&id);
    }
}

/// Ordre stable : l'UI reçoit la même liste pour le même ensemble.
fn sorted<T: Clone, K: Ord>(set: &HashSet<T>, key: impl Fn(&T) -> K) -> Vec<T> {
    let mut out: Vec<T> = set.iter().cloned().collect();
    out.sort_by_key(key);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "4ba30155-d807-446f-9e53-b67520ccfe83";

    #[test]
    fn writes_before_load_are_ignored() {
        let likes = Likes::default();
        likes.set(&LikeTarget::Track { id: TrackId(1) }, true);
        assert!(!likes.replace(LikedKind::Albums(vec![1])));
        assert_eq!(likes.get(), None);
    }

    #[test]
    fn load_then_update_each_kind() {
        let likes = Likes::default();
        likes.load(LikedIds {
            tracks: vec![TrackId(2), TrackId(1)],
            albums: vec![30, 10],
            artists: vec![5],
            playlists: vec![UUID.to_owned()],
            mixes: vec!["m1".to_owned()],
        });
        likes.set(
            &LikeTarget::Mix {
                id: "m2".to_owned(),
            },
            true,
        );
        likes.set(
            &LikeTarget::Mix {
                id: "m1".to_owned(),
            },
            false,
        );
        likes.set(&LikeTarget::Track { id: TrackId(3) }, true);
        likes.set(&LikeTarget::Album { id: 10 }, false);
        likes.set(&LikeTarget::Artist { id: 4 }, true);
        likes.set(
            &LikeTarget::Playlist {
                uuid: UUID.to_owned(),
            },
            false,
        );
        assert_eq!(
            likes.get(),
            Some(LikedIds {
                tracks: vec![TrackId(1), TrackId(2), TrackId(3)],
                albums: vec![30],
                artists: vec![4, 5],
                playlists: vec![],
                mixes: vec!["m2".to_owned()],
            })
        );
        likes.clear();
        assert_eq!(likes.get(), None);
    }

    #[test]
    fn replace_reports_changes_per_kind() {
        let likes = Likes::default();
        likes.load(LikedIds::default());
        assert!(likes.replace(LikedKind::Tracks(vec![TrackId(2), TrackId(1)])));
        assert!(!likes.replace(LikedKind::Tracks(vec![TrackId(1), TrackId(2)])));
        assert!(likes.replace(LikedKind::Artists(vec![7])));
        assert!(!likes.replace(LikedKind::Playlists(vec![])));
        assert!(likes.replace(LikedKind::Mixes(vec!["m".to_owned()])));
        let ids = likes.get().unwrap();
        assert_eq!(ids.tracks, [TrackId(1), TrackId(2)]);
        assert_eq!(ids.artists, [7]);
        assert!(ids.albums.is_empty());
    }
}
