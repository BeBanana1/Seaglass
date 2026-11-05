//! Noms des invités qui ont ajouté des titres, le temps d'une Jam (#25).
//!
//! Gardés ici, en mémoire, et pas dans la file du lecteur : la file reste celle de l'hôte
//! (persistée, sans notion d'invité), et la Jam s'arrête avec l'app. L'UI de l'hôte et la
//! page des invités affichent le nom à côté des titres de la file manuelle.

use std::collections::{HashMap, VecDeque};

/// Longueur maximale d'un nom, en caractères.
pub const MAX_NAME_CHARS: usize = 24;
/// Titres retenus : au-delà, les plus anciens sont oubliés.
const MAX_ENTRIES: usize = 500;

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

#[derive(Debug, Default)]
pub struct Guests {
    by_track: HashMap<u64, String>,
    order: VecDeque<u64>,
}

impl Guests {
    /// Le dernier invité à ajouter un titre lui donne son nom.
    pub fn note(&mut self, track: u64, name: String) {
        if self.by_track.insert(track, name).is_none() {
            self.order.push_back(track);
        }
        while self.order.len() > MAX_ENTRIES {
            if let Some(old) = self.order.pop_front() {
                self.by_track.remove(&old);
            }
        }
    }

    pub fn get(&self, track: u64) -> Option<&str> {
        self.by_track.get(&track).map(String::as_str)
    }

    pub fn snapshot(&self) -> HashMap<u64, String> {
        self.by_track.clone()
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
    fn last_guest_wins_and_entries_are_bounded() {
        let mut guests = Guests::default();
        guests.note(1, "Léa".into());
        guests.note(1, "Max".into());
        assert_eq!(guests.get(1), Some("Max"));
        for id in 2..=(MAX_ENTRIES as u64 + 1) {
            guests.note(id, "x".into());
        }
        assert_eq!(guests.get(1), None);
        assert_eq!(guests.snapshot().len(), MAX_ENTRIES);
    }
}
