//! Raccourcis clavier globaux (#413) : combinaisons choisies par l'utilisateur, actives même
//! quand la fenêtre n'a pas le focus. Ce module ne garde que le réglage et sa lecture
//! (logique pure, testée) ; l'enregistrement auprès du système est dans
//! `integrations::hotkeys` (bureau).
//!
//! Une combinaison est écrite comme la construit l'UI à partir de `KeyboardEvent.code` :
//! modificateurs puis touche, séparés par `+` (`Ctrl+Alt+Space`, `Ctrl+Shift+KeyL`). C'est le
//! format que lit `global-hotkey`.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GlobalAction {
    PlayPause,
    Next,
    Previous,
    VolumeUp,
    VolumeDown,
    /// Aimer ou ne plus aimer le titre en cours : exécuté par l'UI (registre des actions).
    Like,
    ShowWindow,
}

impl GlobalAction {
    pub const ALL: [GlobalAction; 7] = [
        GlobalAction::PlayPause,
        GlobalAction::Next,
        GlobalAction::Previous,
        GlobalAction::VolumeUp,
        GlobalAction::VolumeDown,
        GlobalAction::Like,
        GlobalAction::ShowWindow,
    ];

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| {
            serde_json::to_value(a)
                .ok()
                .and_then(|v| v.as_str().map(|s| s == key))
                == Some(true)
        })
    }
}

/// Désactivés par défaut ; les combinaisons proposées passent toutes par Ctrl+Alt, que les
/// raccourcis internes de l'app n'utilisent pas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GlobalShortcuts {
    pub enabled: bool,
    /// Action sans combinaison : absente.
    #[serde(deserialize_with = "bindings")]
    pub bindings: BTreeMap<GlobalAction, String>,
}

impl Default for GlobalShortcuts {
    fn default() -> Self {
        let bindings = [
            (GlobalAction::PlayPause, "Ctrl+Alt+Space"),
            (GlobalAction::Next, "Ctrl+Alt+ArrowRight"),
            (GlobalAction::Previous, "Ctrl+Alt+ArrowLeft"),
            (GlobalAction::VolumeUp, "Ctrl+Alt+ArrowUp"),
            (GlobalAction::VolumeDown, "Ctrl+Alt+ArrowDown"),
            (GlobalAction::Like, "Ctrl+Alt+KeyL"),
            (GlobalAction::ShowWindow, "Ctrl+Alt+KeyS"),
        ];
        Self {
            enabled: false,
            bindings: bindings
                .into_iter()
                .map(|(a, k)| (a, k.to_owned()))
                .collect(),
        }
    }
}

/// Longueur maximale d'une combinaison (`Ctrl+Alt+Shift+Super+NumpadMultiply` en fait 35).
const MAX_LEN: usize = 64;

/// Modificateurs reconnus ; Maj seule ne suffit pas (elle sert à écrire).
const STRONG_MODIFIERS: [&str; 3] = ["CTRL", "ALT", "SUPER"];
const MODIFIERS: [&str; 4] = ["CTRL", "ALT", "SUPER", "SHIFT"];

/// Combinaison bien formée : modificateurs connus d'abord, dont Ctrl, Alt ou Super, puis une
/// seule touche. Que la touche existe, c'est `global-hotkey` qui le dira à l'enregistrement.
pub fn is_valid(keys: &str) -> bool {
    if keys.is_empty() || keys.len() > MAX_LEN || !keys.is_ascii() {
        return false;
    }
    let parts: Vec<String> = keys.split('+').map(str::to_ascii_uppercase).collect();
    let Some((key, mods)) = parts.split_last() else {
        return false;
    };
    if key.is_empty()
        || !key.chars().all(|c| c.is_ascii_alphanumeric())
        || MODIFIERS.contains(&key.as_str())
    {
        return false;
    }
    let mut seen = Vec::new();
    for m in mods {
        if !MODIFIERS.contains(&m.as_str()) || seen.contains(m) {
            return false;
        }
        seen.push(m.clone());
    }
    seen.iter().any(|m| STRONG_MODIFIERS.contains(&m.as_str()))
}

/// Même combinaison écrite autrement (casse, ordre des modificateurs).
fn canonical(keys: &str) -> (Vec<String>, String) {
    let mut parts: Vec<String> = keys.split('+').map(str::to_ascii_uppercase).collect();
    let key = parts.pop().unwrap_or_default();
    parts.sort();
    (parts, key)
}

/// Lecture tolérante : action inconnue (version plus récente), combinaison mal formée ou déjà
/// prise par une action précédente sont laissées de côté plutôt que de faire perdre tous les
/// réglages d'interface.
fn bindings<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<GlobalAction, String>, D::Error> {
    let raw = BTreeMap::<String, String>::deserialize(d)?;
    Ok(clean(raw))
}

fn clean(raw: BTreeMap<String, String>) -> BTreeMap<GlobalAction, String> {
    let mut out = BTreeMap::new();
    let mut taken = Vec::new();
    for action in GlobalAction::ALL {
        let Some(keys) = raw
            .iter()
            .find(|(k, _)| GlobalAction::from_key(k) == Some(action))
            .map(|(_, v)| v.trim())
        else {
            continue;
        };
        if !is_valid(keys) {
            continue;
        }
        let c = canonical(keys);
        if taken.contains(&c) {
            continue;
        }
        taken.push(c);
        out.insert(action, keys.to_owned());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_by_default_with_one_combination_per_action() {
        let d = GlobalShortcuts::default();
        assert!(!d.enabled);
        assert_eq!(d.bindings.len(), GlobalAction::ALL.len());
        assert!(d.bindings.values().all(|k| is_valid(k)));
    }

    #[test]
    fn needs_a_strong_modifier_and_one_key() {
        assert!(is_valid("Ctrl+Alt+Space"));
        assert!(is_valid("Super+KeyM"));
        assert!(is_valid("alt+shift+F5"));
        assert!(!is_valid("Shift+KeyA"), "Maj seule sert à écrire");
        assert!(!is_valid("KeyA"));
        assert!(!is_valid("Ctrl+Alt"));
        assert!(!is_valid("Ctrl+Ctrl+KeyA"));
        assert!(!is_valid("Ctrl+Hyper+KeyA"));
        assert!(!is_valid("Ctrl+"));
        assert!(!is_valid("Ctrl+Key A"));
        assert!(!is_valid(&format!("Ctrl+{}", "A".repeat(80))));
    }

    #[test]
    fn reads_what_the_ui_sends() {
        let json =
            r#"{"enabled":true,"bindings":{"playPause":"Ctrl+Alt+KeyP","like":"Ctrl+Shift+KeyL"}}"#;
        let s: GlobalShortcuts = serde_json::from_str(json).unwrap();
        assert!(s.enabled);
        assert_eq!(s.bindings.len(), 2);
        assert_eq!(s.bindings[&GlobalAction::Like], "Ctrl+Shift+KeyL");
        assert_eq!(serde_json::to_string(&s).unwrap(), json);
    }

    #[test]
    fn drops_unknown_invalid_and_duplicate_entries() {
        let json = r#"{"bindings":{
            "playPause":"Ctrl+Alt+KeyP",
            "next":"alt+ctrl+keyp",
            "previous":"KeyQ",
            "teleport":"Ctrl+Alt+KeyT",
            "showWindow":"Ctrl+Alt+KeyS"
        }}"#;
        let s: GlobalShortcuts = serde_json::from_str(json).unwrap();
        assert!(!s.enabled);
        let actions: Vec<_> = s.bindings.keys().copied().collect();
        assert_eq!(actions, [GlobalAction::PlayPause, GlobalAction::ShowWindow]);
    }

    #[test]
    fn missing_field_takes_the_defaults() {
        let s: GlobalShortcuts = serde_json::from_str("{}").unwrap();
        assert_eq!(s, GlobalShortcuts::default());
    }
}
