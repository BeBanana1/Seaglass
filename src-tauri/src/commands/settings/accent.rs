//! Accent colors chosen (#23), one per theme (#250). Pure logic.
//!
//! `UiSettings::accents` maps a theme to its `#rrggbb` color; a theme not listed keeps
//! the accent of its CSS. The old single `accent` field (before 0.4.104) is only read,
//! then filed under the theme saved with it (`UiSettings::migrate_accent`).

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer};

use super::Theme;
use crate::i18n::tr;

pub type Accents = BTreeMap<Theme, String>;

/// `#rrggbb` lowercased, or `None`: the value goes into the CSS as is.
fn normalize(color: &str) -> Option<String> {
    let hex = color.strip_prefix('#')?;
    (hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| color.to_ascii_lowercase())
}

fn invalid<E: serde::de::Error>() -> E {
    E::custom(tr(
        "couleur d'accent invalide (attendu #rrggbb)",
        "invalid accent color (expected #rrggbb)",
    ))
}

/// Deserializes `accents`, refusing any color that is not `#rrggbb`.
pub fn accents<'de, D: Deserializer<'de>>(d: D) -> Result<Accents, D::Error> {
    Accents::deserialize(d)?
        .into_iter()
        .map(|(theme, color)| normalize(&color).map(|c| (theme, c)).ok_or_else(invalid))
        .collect()
}

/// Old `accent` field: same validation, `null` accepted.
pub fn legacy<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    match Option::<String>::deserialize(d)? {
        None => Ok(None),
        Some(color) => normalize(&color).map(Some).ok_or_else(invalid),
    }
}

#[cfg(test)]
mod tests {
    use super::super::UiSettings;
    use super::*;

    #[test]
    fn accents_are_validated_and_lowercased() {
        let s: UiSettings =
            serde_json::from_str(r##"{"accents":{"studio":"#FF5C8A","acidwave":"#c8ff00"}}"##)
                .unwrap();
        assert_eq!(
            s.accents.get(&Theme::Studio).map(String::as_str),
            Some("#ff5c8a")
        );
        assert_eq!(
            s.accents.get(&Theme::Acidwave).map(String::as_str),
            Some("#c8ff00")
        );
        for bad in [
            r#""ff5c8a""#,
            r##""#ff5c8""##,
            r#""red""#,
            r##""#ff5c8a;color:red""##,
            "1",
        ] {
            let json = format!(r#"{{"accents":{{"studio":{bad}}}}}"#);
            assert!(serde_json::from_str::<UiSettings>(&json).is_err(), "{bad}");
        }
        assert!(serde_json::from_str::<UiSettings>(r##"{"accents":{"nope":"#ffffff"}}"##).is_err());
    }

    #[test]
    fn legacy_accent_moves_to_its_theme() {
        let mut s: UiSettings =
            serde_json::from_str(r##"{"theme":"studio","accent":"#FF5C8A"}"##).unwrap();
        s.migrate_accent();
        assert_eq!(
            s.accents.get(&Theme::Studio).map(String::as_str),
            Some("#ff5c8a")
        );
        assert_eq!(s.accents.len(), 1);
        // No longer written: only the table goes to the database and to the UI.
        let json = serde_json::to_string(&s).unwrap();
        assert!(!json.contains(r#""accent":"#), "{json}");
        assert!(
            json.contains(r##""accents":{"studio":"#ff5c8a"}"##),
            "{json}"
        );
    }

    #[test]
    fn legacy_accent_does_not_overwrite_a_theme_color() {
        let mut s: UiSettings = serde_json::from_str(
            r##"{"theme":"studio","accent":"#ff5c8a","accents":{"studio":"#3ddc97"}}"##,
        )
        .unwrap();
        s.migrate_accent();
        assert_eq!(
            s.accents.get(&Theme::Studio).map(String::as_str),
            Some("#3ddc97")
        );
        assert!(serde_json::from_str::<UiSettings>(r#"{"accent":"red"}"#).is_err());
        let s: UiSettings = serde_json::from_str(r#"{"accent":null}"#).unwrap();
        assert!(s.accents.is_empty());
    }
}
