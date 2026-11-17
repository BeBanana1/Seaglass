//! Règles de l'hôte pour la Jam (#288) : logique pure, testée sans lecteur ni réseau.
//!
//! Les règles sont retenues d'une Jam à l'autre (réglage `jam.rules`) ; la suspension des
//! ajouts et les exclusions restent propres à chaque Jam (`Core`).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::player::volume::MAX_LEVEL;

/// Plus grande limite de titres en attente par invité acceptée.
pub const MAX_PENDING_LIMIT: u32 = 50;
/// Titres venus de la Jam en attente dans la file manuelle, tous invités confondus (#256) :
/// une adresse qui tire des jetons à la volée ne remplit pas la file de l'hôte.
pub const MAX_JAM_PENDING: usize = 100;
/// Plafond du volume réglé par les invités, par défaut (#449).
pub const DEFAULT_GUEST_VOLUME_MAX: u8 = 80;
/// Votes nécessaires pour passer un titre (#433) : par défaut, bornes.
pub const DEFAULT_SKIP_VOTES: u8 = 3;
pub const MIN_SKIP_VOTES: u8 = 2;
pub const MAX_SKIP_VOTES: u8 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct JamRules {
    /// Les invités peuvent mettre en pause et relancer (#222).
    pub guest_controls: bool,
    /// Titres en attente au plus par invité ; `None` = sans limite.
    pub max_pending: Option<u32>,
    /// Les invités peuvent ajouter un album entier (#223).
    pub albums: bool,
    /// Les invités peuvent régler le volume (#449), désactivé par défaut.
    pub guest_volume: bool,
    /// Volume le plus haut qu'un invité peut demander (1 à 100).
    pub guest_volume_max: u8,
    /// Les invités peuvent voter pour passer le titre en cours (#433), désactivé par défaut.
    pub skip_vote: bool,
    /// Votes qui passent le titre (2 à 10) : un seuil fixe, l'hôte ne sait pas combien
    /// d'invités regardent la page.
    pub skip_votes: u8,
}

impl Default for JamRules {
    fn default() -> Self {
        Self {
            guest_controls: true,
            max_pending: None,
            albums: true,
            guest_volume: false,
            guest_volume_max: DEFAULT_GUEST_VOLUME_MAX,
            skip_vote: false,
            skip_votes: DEFAULT_SKIP_VOTES,
        }
    }
}

impl JamRules {
    /// Limite ramenée entre 1 et `MAX_PENDING_LIMIT` (0 = sans limite), plafond du volume
    /// entre 1 et 100, seuil du vote entre 2 et 10.
    pub fn sanitized(self) -> Self {
        Self {
            max_pending: self
                .max_pending
                .filter(|n| *n > 0)
                .map(|n| n.min(MAX_PENDING_LIMIT)),
            guest_volume_max: self.guest_volume_max.clamp(1, MAX_LEVEL),
            skip_votes: self.skip_votes.clamp(MIN_SKIP_VOTES, MAX_SKIP_VOTES),
            ..self
        }
    }

    /// Niveau appliqué pour la demande `level` d'un invité : ramené au plafond.
    pub fn guest_level(&self, level: u8) -> u8 {
        level.min(self.guest_volume_max)
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
    /// La Jam a atteint sa part de la file manuelle (`MAX_JAM_PENDING`, #256).
    QueueFull,
}

impl Refusal {
    pub fn kind(self) -> (u16, &'static str) {
        match self {
            Self::Duplicate => (409, "duplicate"),
            Self::Full => (429, "full"),
            Self::QueueFull => (429, "queue"),
        }
    }
}

/// Place restante de la Jam dans la file quand `pending` de ses titres y attendent.
pub fn jam_room(pending: usize) -> usize {
    MAX_JAM_PENDING.saturating_sub(pending)
}

/// Titres d'un ajout à mettre dans la file : sans ceux déjà en attente dans la file manuelle
/// ni les répétitions, tronqués à la place restante de l'invité (`room`, `None` = sans limite)
/// et à celle de la Jam (`jam_room`).
pub fn admit(
    candidates: &[u64],
    queued: &[u64],
    room: Option<usize>,
    jam_room: usize,
) -> Result<Vec<u64>, Refusal> {
    let mut seen: HashSet<u64> = queued.iter().copied().collect();
    let fresh: Vec<u64> = candidates
        .iter()
        .copied()
        .filter(|id| seen.insert(*id))
        .collect();
    if fresh.is_empty() {
        return Err(Refusal::Duplicate);
    }
    if jam_room == 0 {
        return Err(Refusal::QueueFull);
    }
    match room {
        Some(0) => Err(Refusal::Full),
        room => Ok(fresh
            .into_iter()
            .take(room.unwrap_or(usize::MAX).min(jam_room))
            .collect()),
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
        // Réglage d'avant #449 : volume fermé aux invités, plafond par défaut.
        assert!(!saved.guest_volume);
        assert_eq!(saved.guest_volume_max, DEFAULT_GUEST_VOLUME_MAX);
        // Réglage d'avant #433 : vote fermé, seuil par défaut.
        assert!(!saved.skip_vote);
        assert_eq!(saved.skip_votes, DEFAULT_SKIP_VOTES);
    }

    #[test]
    fn skip_threshold_is_bounded() {
        let rules = |n| JamRules {
            skip_votes: n,
            ..JamRules::default()
        };
        assert_eq!(rules(0).sanitized().skip_votes, MIN_SKIP_VOTES);
        assert_eq!(rules(1).sanitized().skip_votes, MIN_SKIP_VOTES);
        assert_eq!(rules(5).sanitized().skip_votes, 5);
        assert_eq!(rules(200).sanitized().skip_votes, MAX_SKIP_VOTES);
    }

    #[test]
    fn guest_volume_is_capped() {
        let rules = |max| JamRules {
            guest_volume: true,
            guest_volume_max: max,
            ..JamRules::default()
        };
        assert_eq!(rules(0).sanitized().guest_volume_max, 1);
        assert_eq!(rules(250).sanitized().guest_volume_max, MAX_LEVEL);
        assert_eq!(rules(80).guest_level(95), 80);
        assert_eq!(rules(80).guest_level(30), 30);
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
        let all = MAX_JAM_PENDING;
        assert_eq!(admit(&[1], &[], None, all), Ok(vec![1]));
        assert_eq!(admit(&[1], &[1, 2], None, all), Err(Refusal::Duplicate));
        assert_eq!(admit(&[1, 2, 3, 2, 4], &[2], None, all), Ok(vec![1, 3, 4]));
        assert_eq!(admit(&[1, 2, 3], &[], Some(2), all), Ok(vec![1, 2]));
        assert_eq!(admit(&[1], &[], Some(0), all), Err(Refusal::Full));
        // Déjà en attente : le doublon l'emporte sur la limite (message plus juste).
        assert_eq!(admit(&[1], &[1], Some(0), all), Err(Refusal::Duplicate));
    }

    #[test]
    fn the_jam_share_of_the_queue_is_capped() {
        assert_eq!(jam_room(MAX_JAM_PENDING - 2), 2);
        assert_eq!(jam_room(MAX_JAM_PENDING + 5), 0);
        // Un album est coupé à la plus petite des deux places.
        assert_eq!(admit(&[1, 2, 3, 4], &[], None, 2), Ok(vec![1, 2]));
        assert_eq!(admit(&[1, 2, 3, 4], &[], Some(1), 2), Ok(vec![1]));
        // La file pleine l'emporte sur la limite de l'invité : ce n'est pas de son fait.
        assert_eq!(admit(&[1], &[], Some(0), 0), Err(Refusal::QueueFull));
    }
}
