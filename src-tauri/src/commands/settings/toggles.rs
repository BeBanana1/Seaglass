//! Switches on by default (#452): automatic updates, Jam.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DefaultOn(pub bool);

impl Default for DefaultOn {
    fn default() -> Self {
        Self(true)
    }
}

#[cfg(test)]
mod tests {
    use super::super::UiSettings;
    use super::DefaultOn;

    #[test]
    fn enabled_by_default_and_round_trips() {
        let s: UiSettings = serde_json::from_str(r#"{"theme":"studio"}"#).unwrap();
        assert_eq!(s.auto_updates, DefaultOn(true));
        assert_eq!(s.jam, DefaultOn(true));
        let s: UiSettings = serde_json::from_str(r#"{"autoUpdates":false,"jam":false}"#).unwrap();
        assert_eq!(s.auto_updates, DefaultOn(false));
        assert_eq!(s.jam, DefaultOn(false));
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["autoUpdates"], false);
        assert_eq!(json["jam"], false);
        assert!(serde_json::from_str::<UiSettings>(r#"{"jam":"yes"}"#).is_err());
    }
}
