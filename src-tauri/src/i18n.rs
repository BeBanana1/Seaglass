//! Langue de l'app (#7) : français ou anglais. La langue effective est décidée ici
//! (réglage, sinon langue d'affichage du système) ; l'UI la reçoit et ne fait que l'afficher.
//!
//! Côté Rust, elle sert aux messages montrés à l'utilisateur (erreurs des commandes, toasts)
//! et à la `locale` des requêtes TIDAL dont le contenu est rédigé (accueil, fil, mixes).

use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    Fr,
    En,
}

/// Réglage de l'utilisateur ; `System` suit la langue d'affichage de Windows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguagePref {
    #[default]
    System,
    Fr,
    En,
}

impl LanguagePref {
    pub fn resolve(self) -> Lang {
        match self {
            Self::System => system(),
            Self::Fr => Lang::Fr,
            Self::En => Lang::En,
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn current() -> Lang {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Lang::En,
        _ => Lang::Fr,
    }
}

pub fn set(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

/// Choisit le texte dans la langue courante.
pub fn tr(fr: &'static str, en: &'static str) -> &'static str {
    match current() {
        Lang::Fr => fr,
        Lang::En => en,
    }
}

/// `locale` des requêtes TIDAL : langue des titres de l'accueil, du fil et des mixes.
pub fn tidal_locale() -> &'static str {
    tr("fr_FR", "en_US")
}

/// Message formaté dans la langue courante : `trf!("réseau : {}", "network: {}", e)`.
/// Les deux modèles reçoivent les mêmes arguments.
macro_rules! trf {
    ($fr:literal, $en:literal $(, $arg:expr)* $(,)?) => {
        match $crate::i18n::current() {
            $crate::i18n::Lang::Fr => format!($fr $(, $arg)*),
            $crate::i18n::Lang::En => format!($en $(, $arg)*),
        }
    };
}
pub(crate) use trf;

/// Français si la langue d'affichage du système est le français, anglais sinon.
#[cfg(windows)]
fn system() -> Lang {
    use windows::Win32::Globalization::GetUserDefaultUILanguage;
    // Langue principale = 10 bits de poids faible du LANGID ; LANG_FRENCH = 0x0c.
    // SAFETY : fonction sans argument ni état partagé.
    let langid = unsafe { GetUserDefaultUILanguage() };
    if langid & 0x3ff == 0x0c {
        Lang::Fr
    } else {
        Lang::En
    }
}

#[cfg(not(windows))]
fn system() -> Lang {
    let var = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|v| std::env::var(v).ok().filter(|s| !s.is_empty()))
        .unwrap_or_default();
    if var.starts_with("fr") {
        Lang::Fr
    } else {
        Lang::En
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pref_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&LanguagePref::System).unwrap(),
            "\"system\""
        );
        assert_eq!(
            serde_json::from_str::<LanguagePref>("\"en\"").unwrap(),
            LanguagePref::En
        );
        assert_eq!(serde_json::to_string(&Lang::Fr).unwrap(), "\"fr\"");
        assert!(serde_json::from_str::<LanguagePref>("\"de\"").is_err());
    }

    #[test]
    fn explicit_pref_ignores_system() {
        assert_eq!(LanguagePref::Fr.resolve(), Lang::Fr);
        assert_eq!(LanguagePref::En.resolve(), Lang::En);
    }
}
