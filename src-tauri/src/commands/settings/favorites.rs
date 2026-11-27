//! Favorite themes (#390), at the top of the settings gallery. Pure logic.
//!
//! `UiSettings::favorite_themes` keeps the family identifier (the light theme of
//! Neumorphism or Alloy, not its dark variant: the UI does the grouping), without duplicates.
//! An unknown identifier (settings written by a newer version) is ignored, so as not to
//! make all the settings unreadable.

use serde::de::IntoDeserializer;
use serde::de::value::{Error as ValueError, StrDeserializer};
use serde::{Deserialize, Deserializer};

use super::Theme;

/// Deserializes the list, ignoring unknown themes and duplicates.
pub fn favorites<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Theme>, D::Error> {
    let mut themes = Vec::new();
    for id in Vec::<String>::deserialize(d)? {
        let de: StrDeserializer<'_, ValueError> = id.as_str().into_deserializer();
        if let Ok(theme) = Theme::deserialize(de)
            && !themes.contains(&theme)
        {
            themes.push(theme);
        }
    }
    Ok(themes)
}

#[cfg(test)]
mod tests {
    use super::super::UiSettings;
    use super::*;

    #[test]
    fn unknown_and_duplicate_favorites_are_dropped() {
        let s: UiSettings = serde_json::from_str(
            r#"{"favoriteThemes":["alloy","nope","studio","alloy","neumorph-dark"]}"#,
        )
        .unwrap();
        assert_eq!(
            s.favorite_themes,
            vec![Theme::Alloy, Theme::Studio, Theme::NeumorphDark]
        );
    }

    #[test]
    fn favorites_default_to_empty_and_round_trip() {
        let s: UiSettings = serde_json::from_str("{}").unwrap();
        assert!(s.favorite_themes.is_empty());
        let s = UiSettings {
            favorite_themes: vec![Theme::Terminal],
            ..UiSettings::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains(r#""favoriteThemes":["terminal"]"#), "{json}");
    }

    #[test]
    fn favorites_must_be_a_list() {
        assert!(serde_json::from_str::<UiSettings>(r#"{"favoriteThemes":"alloy"}"#).is_err());
    }
}
