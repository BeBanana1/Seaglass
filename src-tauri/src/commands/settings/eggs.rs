//! Clins d'œil de l'interface (#399) : un simple interrupteur, activé par défaut. Les
//! déclencheurs (dates, recherches) sont de la présentation pure et vivent dans le frontend
//! (`src/lib/eggs/`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EasterEggs(pub bool);

impl Default for EasterEggs {
    fn default() -> Self {
        Self(true)
    }
}

#[cfg(test)]
mod tests {
    use super::super::UiSettings;
    use super::EasterEggs;

    #[test]
    fn enabled_by_default_and_round_trips() {
        let s: UiSettings = serde_json::from_str(r#"{"theme":"studio"}"#).unwrap();
        assert_eq!(s.easter_eggs, EasterEggs(true));
        let s: UiSettings = serde_json::from_str(r#"{"easterEggs":false}"#).unwrap();
        assert_eq!(s.easter_eggs, EasterEggs(false));
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["easterEggs"], false);
        assert!(serde_json::from_str::<UiSettings>(r#"{"easterEggs":"yes"}"#).is_err());
    }
}
