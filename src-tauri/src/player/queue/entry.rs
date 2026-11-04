//! Titre de la file avec son origine : titre courant, historique et file manuelle.

use serde::{Deserialize, Serialize};

use crate::tidal::models::TrackId;

/// D'où vient un titre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Origin {
    /// Du contexte en cours : le curseur `pos` le désigne.
    Context,
    /// Ajouté par l'utilisateur à la file manuelle (« Lire ensuite », « Ajouter à la file »).
    Manual,
    /// Ni l'un ni l'autre : titre d'un contexte remplacé depuis, ou titre courant après
    /// « Vider la file ».
    Detached,
}

/// Dans la file manuelle, une origine autre que `Manual` marque un titre quitté par
/// « Précédent », remis en tête pour que « Suivant » y ramène : un nouveau contexte l'écarte
/// (#166 : ces titres s'accumulaient et passaient avant toute nouvelle liste).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "StoredEntry")]
pub(super) struct Entry {
    pub id: TrackId,
    pub origin: Origin,
}

impl Entry {
    pub fn new(id: TrackId, origin: Origin) -> Self {
        Self { id, origin }
    }

    pub fn in_context(self) -> bool {
        self.origin == Origin::Context
    }

    /// Ajouté par l'utilisateur, par opposition à un titre rappelé par « Précédent ».
    pub fn queued_by_user(self) -> bool {
        self.origin == Origin::Manual
    }
}

/// `Entry` tel qu'enregistré. Avant 0.4.64 : un simple identifiant dans la file manuelle,
/// et `{id, from_context}` ailleurs (faux = file manuelle, sans distinguer un titre détaché).
#[derive(Deserialize)]
#[serde(untagged)]
enum StoredEntry {
    Id(TrackId),
    Full {
        id: TrackId,
        origin: Option<Origin>,
        #[serde(default)]
        from_context: bool,
    },
}

impl From<StoredEntry> for Entry {
    fn from(s: StoredEntry) -> Self {
        match s {
            StoredEntry::Id(id) => Self::new(id, Origin::Manual),
            StoredEntry::Full {
                id,
                origin,
                from_context,
            } => {
                let legacy = if from_context {
                    Origin::Context
                } else {
                    Origin::Manual
                };
                Self::new(id, origin.unwrap_or(legacy))
            }
        }
    }
}
