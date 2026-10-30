//! Disposition de l'accueil choisie par l'utilisateur : ordre des sections et sections
//! masquées (#21, #22). Logique pure, sans réseau ni base : le réglage est stocké dans
//! `UiSettings::home` et appliqué au flux à chaque chargement de l'accueil.
//!
//! Les sections sont identifiées par `HomeRow::key` (le `moduleId` de l'API). Le flux
//! change d'un jour à l'autre : une section inconnue apparaît à sa place par défaut,
//! une section absente garde sa position enregistrée pour le jour où elle revient.

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::tidal::models::HomeRow;

/// Nombre maximal de sections envoyées par l'UI (le flux en compte une quinzaine).
pub const MAX_SECTIONS: usize = 100;
/// Longueur maximale d'une clé (`moduleId` ou titre normalisé).
pub const MAX_KEY_LEN: usize = 200;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HomeLayout {
    /// Clés dans l'ordre choisi. Vide = ordre du flux.
    pub order: Vec<String>,
    /// Clés des sections masquées.
    pub hidden: BTreeSet<String>,
}

/// Ligne de l'accueil telle qu'affichée : la ligne du flux et son état de masquage
/// (l'UI a besoin des sections masquées pour pouvoir les réafficher).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeSection {
    #[serde(flatten)]
    pub row: HomeRow,
    pub hidden: bool,
}

/// Section telle que l'UI l'affiche en mode « Personnaliser », dans l'ordre de l'écran.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionPref {
    pub key: String,
    pub hidden: bool,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LayoutError {
    #[error("trop de sections ({0}, maximum {MAX_SECTIONS})")]
    TooMany(usize),
    #[error("clé de section invalide")]
    InvalidKey,
}

impl HomeLayout {
    /// Ordonne les lignes du flux selon la disposition enregistrée et marque les masquées.
    ///
    /// Plusieurs lignes peuvent partager une clé (deux « Parce que vous avez écouté ») :
    /// elles restent groupées à la place de la première.
    pub fn arrange(&self, rows: Vec<HomeRow>) -> Vec<HomeSection> {
        // Groupes de lignes par clé, dans l'ordre du flux.
        let mut groups: Vec<(String, Vec<HomeRow>)> = Vec::new();
        for row in rows {
            match groups.iter_mut().find(|(k, _)| *k == row.key) {
                Some((_, g)) => g.push(row),
                None => groups.push((row.key.clone(), vec![row])),
            }
        }
        let feed: Vec<&str> = groups.iter().map(|(k, _)| k.as_str()).collect();
        let keys = merge_order(&feed, &self.order);
        let mut by_key: HashMap<String, Vec<HomeRow>> = groups.into_iter().collect();
        keys.iter()
            .filter_map(|k| by_key.remove(k))
            .flatten()
            .map(|row| HomeSection {
                hidden: self.hidden.contains(&row.key),
                row,
            })
            .collect()
    }

    /// Enregistre l'ordre et le masquage des sections affichées. Les sections absentes de
    /// `shown` (pas dans le flux du jour) gardent leur position et leur état.
    pub fn update(&mut self, shown: &[SectionPref]) -> Result<(), LayoutError> {
        if shown.len() > MAX_SECTIONS {
            return Err(LayoutError::TooMany(shown.len()));
        }
        if shown
            .iter()
            .any(|s| s.key.is_empty() || s.key.chars().count() > MAX_KEY_LEN)
        {
            return Err(LayoutError::InvalidKey);
        }
        let mut seen = BTreeSet::new();
        let shown_keys: Vec<&str> = shown
            .iter()
            .map(|s| s.key.as_str())
            .filter(|k| seen.insert(*k))
            .collect();
        // Base = ancien ordre + nouvelles clés : les absentes s'y raccrochent à leur voisine.
        let mut base: Vec<&str> = self.order.iter().map(String::as_str).collect();
        base.extend(
            shown_keys
                .iter()
                .filter(|k| !self.order.iter().any(|o| o == *k)),
        );
        self.order = merge_order(&base, &owned(&shown_keys));
        self.hidden.retain(|k| !seen.contains(k.as_str()));
        self.hidden
            .extend(shown.iter().filter(|s| s.hidden).map(|s| s.key.clone()));
        Ok(())
    }
}

fn owned(keys: &[&str]) -> Vec<String> {
    keys.iter().map(|k| (*k).to_owned()).collect()
}

/// Fusionne un ordre de `base` (l'ordre par défaut) avec un ordre `preferred` :
/// les clés de `base` présentes dans `preferred` suivent `preferred` ; chaque autre clé de
/// `base` est insérée juste après celle qui la précède dans `base` (en tête si aucune).
/// Les clés de `preferred` absentes de `base` ne sont pas reprises.
fn merge_order(base: &[&str], preferred: &[String]) -> Vec<String> {
    let rank: HashMap<&str, usize> = preferred
        .iter()
        .enumerate()
        .map(|(i, k)| (k.as_str(), i))
        .collect();
    let mut known: Vec<&str> = base
        .iter()
        .copied()
        .filter(|k| rank.contains_key(k))
        .collect();
    known.sort_by_key(|k| rank[k]);
    let mut out: Vec<String> = owned(&known);
    for (i, key) in base.iter().enumerate() {
        if rank.contains_key(key) {
            continue;
        }
        let at = match i.checked_sub(1).map(|p| base[p]) {
            Some(prev) => out.iter().position(|k| k == prev).map_or(0, |p| p + 1),
            None => 0,
        };
        out.insert(at, (*key).to_owned());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(key: &str) -> HomeRow {
        HomeRow {
            key: key.into(),
            title: key.to_lowercase(),
            items: Vec::new(),
        }
    }

    fn feed(keys: &[&str]) -> Vec<HomeRow> {
        keys.iter().map(|k| row(k)).collect()
    }

    fn keys(sections: &[HomeSection]) -> Vec<&str> {
        sections.iter().map(|s| s.row.key.as_str()).collect()
    }

    fn prefs(items: &[(&str, bool)]) -> Vec<SectionPref> {
        items
            .iter()
            .map(|(k, h)| SectionPref {
                key: (*k).into(),
                hidden: *h,
            })
            .collect()
    }

    #[test]
    fn default_layout_keeps_feed_order() {
        let out = HomeLayout::default().arrange(feed(&["A", "B", "C"]));
        assert_eq!(keys(&out), ["A", "B", "C"]);
        assert!(out.iter().all(|s| !s.hidden));
    }

    #[test]
    fn saved_order_and_hidden_are_applied() {
        let layout = HomeLayout {
            order: vec!["C".into(), "A".into(), "B".into()],
            hidden: ["A".to_owned()].into(),
        };
        let out = layout.arrange(feed(&["A", "B", "C"]));
        assert_eq!(keys(&out), ["C", "A", "B"]);
        assert_eq!(
            out.iter().map(|s| s.hidden).collect::<Vec<_>>(),
            [false, true, false]
        );
    }

    #[test]
    fn unknown_sections_appear_after_their_default_neighbour() {
        let layout = HomeLayout {
            order: vec!["C".into(), "A".into()],
            hidden: BTreeSet::new(),
        };
        // N1 suit A dans le flux, N0 est en tête, N2 suit N1.
        let out = layout.arrange(feed(&["N0", "A", "N1", "N2", "C"]));
        assert_eq!(keys(&out), ["N0", "C", "A", "N1", "N2"]);
    }

    #[test]
    fn missing_sections_are_skipped() {
        let layout = HomeLayout {
            order: vec!["GONE".into(), "B".into(), "A".into()],
            hidden: ["GONE".to_owned()].into(),
        };
        assert_eq!(keys(&layout.arrange(feed(&["A", "B"]))), ["B", "A"]);
    }

    #[test]
    fn duplicate_keys_stay_grouped() {
        let layout = HomeLayout {
            order: vec!["B".into(), "BECAUSE".into()],
            hidden: ["BECAUSE".to_owned()].into(),
        };
        let out = layout.arrange(feed(&["BECAUSE", "B", "BECAUSE"]));
        assert_eq!(keys(&out), ["B", "BECAUSE", "BECAUSE"]);
        assert!(out[1].hidden && out[2].hidden);
    }

    #[test]
    fn update_keeps_absent_sections_in_place() {
        let mut layout = HomeLayout {
            order: vec!["A".into(), "GONE".into(), "B".into(), "C".into()],
            hidden: ["GONE".to_owned(), "A".to_owned()].into(),
        };
        // Le flux du jour n'a pas GONE ; l'utilisateur remonte C et réaffiche A.
        layout
            .update(&prefs(&[("C", false), ("A", false), ("B", true)]))
            .unwrap();
        assert_eq!(layout.order, ["C", "A", "GONE", "B"]);
        assert_eq!(
            layout.hidden,
            ["B".to_owned(), "GONE".to_owned()].into_iter().collect()
        );
    }

    #[test]
    fn update_from_empty_layout_and_dedup() {
        let mut layout = HomeLayout::default();
        layout
            .update(&prefs(&[("B", false), ("A", true), ("B", false)]))
            .unwrap();
        assert_eq!(layout.order, ["B", "A"]);
        assert!(layout.arrange(feed(&["A", "B"]))[1].hidden);
    }

    #[test]
    fn update_rejects_bad_input() {
        let mut layout = HomeLayout::default();
        assert_eq!(
            layout.update(&prefs(&[("", false)])),
            Err(LayoutError::InvalidKey)
        );
        let long = "x".repeat(MAX_KEY_LEN + 1);
        assert_eq!(
            layout.update(&prefs(&[(long.as_str(), false)])),
            Err(LayoutError::InvalidKey)
        );
        let many: Vec<SectionPref> = (0..=MAX_SECTIONS)
            .map(|i| SectionPref {
                key: i.to_string(),
                hidden: false,
            })
            .collect();
        assert_eq!(
            layout.update(&many),
            Err(LayoutError::TooMany(MAX_SECTIONS + 1))
        );
        assert_eq!(layout, HomeLayout::default());
    }

    #[test]
    fn serde_defaults_and_shape() {
        let l: HomeLayout = serde_json::from_str("{}").unwrap();
        assert_eq!(l, HomeLayout::default());
        let l: HomeLayout = serde_json::from_str(r#"{"order":["A"],"hidden":["B","B"]}"#).unwrap();
        assert_eq!(l.order, ["A"]);
        assert_eq!(l.hidden.len(), 1);
        let section = HomeSection {
            row: row("A"),
            hidden: true,
        };
        let v = serde_json::to_value(&section).unwrap();
        assert_eq!(v["key"], "A");
        assert_eq!(v["hidden"], true);
        assert!(v["items"].is_array());
    }
}
