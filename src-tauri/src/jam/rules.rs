//! Règles de l'hôte pour la Jam (#288) : logique pure, testée sans lecteur ni réseau.
//!
//! Les règles sont retenues d'une Jam à l'autre (réglage `jam.rules`) ; la suspension des
//! ajouts et les exclusions restent propres à chaque Jam (`Core`).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// Plus grande limite de titres en attente par invité acceptée.
pub const MAX_PENDING_LIMIT: u32 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct JamRules {
    /// Les invités peuvent mettre en pause et relancer (#222).
    pub guest_controls: bool,
    /// Titres en attente au plus par invité ; `None` = sans limite.
    pub max_pending: Option<u32>,
    /// Les invités peuvent ajouter un album entier (#223).
    pub albums: bool,
}

impl Default for JamRules {
    fn default() -> Self {
        Self {
            guest_controls: true,
            max_pending: None,
            albums: true,
        }
    }
}

impl JamRules {
    /// Limite ramenée entre 1 et `MAX_PENDING_LIMIT` ; 0 = sans limite.
    pub fn sanitized(self) -> Self {
        Self {
            max_pending: self
                .max_pending
                .filter(|n| *n > 0)
                .map(|n| n.min(MAX_PENDING_LIMIT)),
            ..self
        }
    }

    /// Place restante d'un invité qui a déjà `pending` titres en attente ; `None` = sans limite.
    pub fn room(&self, pending: usize) -> Option<usize> {
        self.max_pending
            .map(|max| (max as usize).saturating_sub(pending))
    }
}

/// Pourquoi un ajout est refusé ; le nom est le type d'erreur que la page reçoit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Tous les titres sont déjà dans la file manuelle.
    Duplicate,
    /// L'invité a atteint sa limite de titres en attente.
    Full,
}

impl Refusal {
    pub fn kind(self) -> (u16, &'static str) {
        match self {
            Self::Duplicate => (409, "duplicate"),
            Self::Full => (429, "full"),
        }
    }
}

/// Titres d'un ajout à mettre dans la file : sans ceux déjà en attente dans la file manuelle
/// ni les répétitions, tronqués à la place restante de l'invité (`room`, `None` = sans limite).
pub fn admit(candidates: &[u64], queued: &[u64], room: Option<usize>) -> Result<Vec<u64>, Refusal> {
    let mut seen: HashSet<u64> = queued.iter().copied().collect();
    let fresh: Vec<u64> = candidates
        .iter()
        .copied()
        .filter(|id| seen.insert(*id))
        .collect();
    if fresh.is_empty() {
        return Err(Refusal::Duplicate);
    }
    match room {
        Some(0) => Err(Refusal::Full),
        Some(n) => Ok(fresh.into_iter().take(n).collect()),
        None => Ok(fresh),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_keep_the_jam_open() {
        let rules = JamRules::default();
        assert!(rules.guest_controls && rules.albums);
        assert_eq!(rules.room(100), None);
        // Réglage d'avant #288 ou incomplet : valeurs par défaut.
        let saved: JamRules = serde_json::from_str(r#"{"albums":false}"#).unwrap();
        assert!(saved.guest_controls);
        assert!(!saved.albums);
    }

    #[test]
    fn limits_are_bounded() {
        let rules = |max| JamRules {
            max_pending: max,
            ..JamRules::default()
        };
        assert_eq!(rules(Some(0)).sanitized().max_pending, None);
        assert_eq!(
            rules(Some(999)).sanitized().max_pending,
            Some(MAX_PENDING_LIMIT)
        );
        assert_eq!(rules(Some(3)).room(1), Some(2));
        assert_eq!(rules(Some(3)).room(5), Some(0));
    }

    #[test]
    fn duplicates_are_skipped_and_the_rest_fits_the_room() {
        assert_eq!(admit(&[1], &[], None), Ok(vec![1]));
        assert_eq!(admit(&[1], &[1, 2], None), Err(Refusal::Duplicate));
        assert_eq!(admit(&[1, 2, 3, 2, 4], &[2], None), Ok(vec![1, 3, 4]));
        assert_eq!(admit(&[1, 2, 3], &[], Some(2)), Ok(vec![1, 2]));
        assert_eq!(admit(&[1], &[], Some(0)), Err(Refusal::Full));
        // Déjà en attente : le doublon l'emporte sur la limite (message plus juste).
        assert_eq!(admit(&[1], &[1], Some(0)), Err(Refusal::Duplicate));
    }
}
