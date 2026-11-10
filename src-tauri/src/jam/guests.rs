//! Ajouts des invités, le temps d'une Jam (#25) : nom affiché, auteur et lot de chaque titre.
//!
//! Gardés ici, en mémoire, et pas dans la file du lecteur : la file reste celle de l'hôte
//! (persistée, sans notion d'invité), et la Jam s'arrête avec l'app. L'UI de l'hôte et la
//! page des invités affichent le nom à côté des titres de la file manuelle.
//!
//! Un **lot** est un ajout : un titre, ou les titres d'un album (#223). L'auteur est le
//! jeton tiré par le navigateur de l'invité : lui seul peut retirer ses lots. Le prénom
//! n'est pas une identité (n'importe qui peut taper le même).

use std::collections::{HashMap, VecDeque};

use super::members::Members;

/// Longueur maximale d'un nom, en caractères.
pub const MAX_NAME_CHARS: usize = 24;
/// Titres retenus : au-delà, les plus anciens sont oubliés.
const MAX_ENTRIES: usize = 500;
/// Jeton d'invité : hexadécimal, de 16 à 64 caractères.
const TOKEN_MIN: usize = 16;
const TOKEN_MAX: usize = 64;

/// Nom saisi par un invité : espaces réduits, caractères de contrôle retirés, coupé à
/// `MAX_NAME_CHARS`. Vide = pas de nom.
pub fn sanitize_name(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_NAME_CHARS)
        .collect();
    let cleaned = cleaned.trim_end().to_owned();
    (!cleaned.is_empty()).then_some(cleaned)
}

/// Jeton d'un navigateur d'invité, `None` s'il n'a pas la bonne forme (ancienne page :
/// l'ajout reste permis, sans retrait possible).
pub fn sanitize_token(raw: &str) -> Option<String> {
    let ok =
        (TOKEN_MIN..=TOKEN_MAX).contains(&raw.len()) && raw.bytes().all(|b| b.is_ascii_hexdigit());
    ok.then(|| raw.to_ascii_lowercase())
}

#[derive(Debug, Clone, PartialEq)]
struct Added {
    name: Option<String>,
    owner: Option<String>,
    batch: u32,
}

#[derive(Debug, Default)]
pub struct Guests {
    by_track: HashMap<u64, Added>,
    order: VecDeque<u64>,
    next_batch: u32,
    /// Invités qui ont ajouté, et ceux que l'hôte a exclus (#288).
    pub members: Members,
}

impl Guests {
    /// Nouveau lot ; ses titres sont notés ensuite par `note`.
    pub fn new_batch(&mut self) -> u32 {
        self.next_batch = self.next_batch.wrapping_add(1);
        self.next_batch
    }

    /// Le dernier invité à ajouter un titre en devient l'auteur.
    pub fn note(&mut self, track: u64, batch: u32, name: Option<String>, owner: Option<String>) {
        let added = Added { name, owner, batch };
        if self.by_track.insert(track, added).is_none() {
            self.order.push_back(track);
        }
        while self.order.len() > MAX_ENTRIES {
            if let Some(old) = self.order.pop_front() {
                self.by_track.remove(&old);
            }
        }
    }

    pub fn name(&self, track: u64) -> Option<&str> {
        self.by_track.get(&track)?.name.as_deref()
    }

    pub fn batch(&self, track: u64) -> Option<u32> {
        self.by_track.get(&track).map(|a| a.batch)
    }

    /// Retire les titres d'un lot de cet auteur (un seul si `track` est donné) et renvoie
    /// leurs identifiants. Un titre ajouté de nouveau par quelqu'un d'autre ne l'est plus.
    pub fn take(&mut self, owner: &str, batch: u32, track: Option<u64>) -> Vec<u64> {
        let taken: Vec<u64> = self
            .order
            .iter()
            .copied()
            .filter(|id| track.is_none_or(|t| t == *id))
            .filter(|id| {
                self.by_track
                    .get(id)
                    .is_some_and(|a| a.batch == batch && a.owner.as_deref() == Some(owner))
            })
            .collect();
        for id in &taken {
            self.by_track.remove(id);
        }
        self.order.retain(|id| !taken.contains(id));
        taken
    }

    /// Titres de cet invité encore en attente dans la file manuelle (#288).
    pub fn pending(&self, owner: &str, manual: &[u64]) -> usize {
        manual
            .iter()
            .filter(|id| {
                self.by_track
                    .get(id)
                    .is_some_and(|a| a.owner.as_deref() == Some(owner))
            })
            .count()
    }

    /// Retire tous les titres de cet invité, quel que soit le lot (exclusion, #288).
    pub fn take_all(&mut self, owner: &str) -> Vec<u64> {
        let taken: Vec<u64> = self
            .order
            .iter()
            .copied()
            .filter(|id| {
                self.by_track
                    .get(id)
                    .is_some_and(|a| a.owner.as_deref() == Some(owner))
            })
            .collect();
        for id in &taken {
            self.by_track.remove(id);
        }
        self.order.retain(|id| !taken.contains(id));
        taken
    }

    /// Nom de l'invité de chaque titre, pour l'UI de l'hôte.
    pub fn snapshot(&self) -> HashMap<u64, String> {
        self.by_track
            .iter()
            .filter_map(|(id, a)| Some((*id, a.name.clone()?)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_cleaned_and_bounded() {
        assert_eq!(sanitize_name("  Léa  "), Some("Léa".into()));
        assert_eq!(sanitize_name("Jean\n\tLuc"), Some("Jean Luc".into()));
        assert_eq!(sanitize_name(" \u{7}  "), None);
        let long = sanitize_name(&"é".repeat(40)).unwrap();
        assert_eq!(long.chars().count(), MAX_NAME_CHARS);
        // Coupé juste après un espace : pas d'espace final.
        let cut = sanitize_name(&format!("{} b", "a".repeat(MAX_NAME_CHARS - 1))).unwrap();
        assert_eq!(cut, "a".repeat(MAX_NAME_CHARS - 1));
    }

    #[test]
    fn tokens_are_hex_of_bounded_length() {
        assert_eq!(
            sanitize_token("0123456789ABCDEF"),
            Some("0123456789abcdef".into())
        );
        assert_eq!(sanitize_token("0123456789abcde"), None);
        assert_eq!(sanitize_token(&"a".repeat(65)), None);
        assert_eq!(sanitize_token("0123456789abcdeg"), None);
    }

    #[test]
    fn last_guest_wins_and_entries_are_bounded() {
        let mut guests = Guests::default();
        guests.note(1, 1, Some("Léa".into()), None);
        guests.note(1, 2, Some("Max".into()), None);
        assert_eq!(guests.name(1), Some("Max"));
        assert_eq!(guests.batch(1), Some(2));
        guests.note(2, 3, None, None);
        assert!(!guests.snapshot().contains_key(&2));
        for id in 3..=(MAX_ENTRIES as u64 + 1) {
            guests.note(id, 4, Some("x".into()), None);
        }
        assert_eq!(guests.name(1), None);
        assert_eq!(guests.by_track.len(), MAX_ENTRIES);
    }

    #[test]
    fn only_the_owner_takes_a_batch() {
        let (lea, max) = ("a".repeat(32), "b".repeat(32));
        let mut guests = Guests::default();
        let album = guests.new_batch();
        for id in [1, 2, 3] {
            guests.note(id, album, Some("Léa".into()), Some(lea.clone()));
        }
        // Max ajoute de nouveau le titre 3 : il est à lui.
        let single = guests.new_batch();
        guests.note(3, single, Some("Max".into()), Some(max.clone()));

        assert!(guests.take(&max, album, None).is_empty());
        assert_eq!(guests.take(&lea, album, Some(2)), vec![2]);
        assert_eq!(guests.take(&lea, album, None), vec![1]);
        assert_eq!(guests.name(3), Some("Max"));
        assert_eq!(guests.take(&max, single, None), vec![3]);
        assert!(guests.snapshot().is_empty());
    }

    #[test]
    fn pending_counts_only_queued_tracks_of_the_owner() {
        let (lea, max) = ("a".repeat(32), "b".repeat(32));
        let mut guests = Guests::default();
        for id in [1, 2, 3] {
            let batch = guests.new_batch();
            guests.note(id, batch, None, Some(lea.clone()));
        }
        let batch = guests.new_batch();
        guests.note(4, batch, None, Some(max.clone()));
        // Le titre 1 a été lu : il n'est plus dans la file manuelle.
        assert_eq!(guests.pending(&lea, &[2, 3, 4]), 2);
        assert_eq!(guests.pending(&max, &[2, 3, 4]), 1);
        let mut taken = guests.take_all(&lea);
        taken.sort_unstable();
        assert_eq!(taken, vec![1, 2, 3]);
        assert_eq!(guests.pending(&lea, &[2, 3, 4]), 0);
        assert_eq!(guests.batch(4), Some(batch));
    }
}
