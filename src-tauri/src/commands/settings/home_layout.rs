//! Home page layout chosen by the user: order of the sections and hidden
//! sections (#21, #22). Pure logic, with no network or database: the setting is stored in
//! `UiSettings::home` and applied to the feed on every load of the home page.
//!
//! Sections are identified by `HomeRow::key` (the API's `moduleId`). The feed
//! changes from one day to the next: an unknown section appears at its default place,
//! an absent section keeps its saved position for the day it comes back.

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::i18n::{tr, trf};
use crate::tidal::models::HomeRow;

/// Maximum number of sections sent by the UI (the feed has about fifteen).
pub const MAX_SECTIONS: usize = 100;
/// Maximum length of a key (`moduleId` or normalized title).
pub const MAX_KEY_LEN: usize = 200;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HomeLayout {
    /// Keys in the chosen order. Empty = feed order.
    pub order: Vec<String>,
    /// Keys of the hidden sections.
    pub hidden: BTreeSet<String>,
    /// "Featured" banner hidden (#185). It is not a row of the feed: the UI draws it
    /// from the albums of the displayed sections.
    pub featured_hidden: bool,
}

/// Home row as displayed: the feed row and its hiding state
/// (the UI needs the hidden sections to be able to show them again).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeSection {
    #[serde(flatten)]
    pub row: HomeRow,
    pub hidden: bool,
}

/// Section as the UI displays it in "Customize" mode, in screen order.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionPref {
    pub key: String,
    pub hidden: bool,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LayoutError {
    #[error("{}", trf!(
        "trop de sections ({}, maximum {MAX_SECTIONS})",
        "too many sections ({}, maximum {MAX_SECTIONS})",
        .0
    ))]
    TooMany(usize),
    #[error("{}", tr("clé de section invalide", "invalid section key"))]
    InvalidKey,
}

impl HomeLayout {
    /// Orders the feed rows according to the saved layout and marks the hidden ones.
    ///
    /// Several rows can share a key (two "Because you listened to"):
    /// they stay grouped at the place of the first one.
    pub fn arrange(&self, rows: Vec<HomeRow>) -> Vec<HomeSection> {
        // Groups of rows by key, in feed order.
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

    /// Saves the order and hiding of the displayed sections. Sections absent from
    /// `shown` (not in the day's feed) keep their position and state.
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
        // Base = old order + new keys: the absent ones attach to their neighbor.
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

/// Merges an order of `base` (the default order) with an order `preferred`:
/// the keys of `base` present in `preferred` follow `preferred`; every other key of
/// `base` is inserted right after the one that precedes it in `base` (first if none).
/// The keys of `preferred` absent from `base` are not taken.
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
            ..HomeLayout::default()
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
            ..HomeLayout::default()
        };
        // N1 follows A in the feed, N0 is first, N2 follows N1.
        let out = layout.arrange(feed(&["N0", "A", "N1", "N2", "C"]));
        assert_eq!(keys(&out), ["N0", "C", "A", "N1", "N2"]);
    }

    #[test]
    fn missing_sections_are_skipped() {
        let layout = HomeLayout {
            order: vec!["GONE".into(), "B".into(), "A".into()],
            hidden: ["GONE".to_owned()].into(),
            ..HomeLayout::default()
        };
        assert_eq!(keys(&layout.arrange(feed(&["A", "B"]))), ["B", "A"]);
    }

    #[test]
    fn duplicate_keys_stay_grouped() {
        let layout = HomeLayout {
            order: vec!["B".into(), "BECAUSE".into()],
            hidden: ["BECAUSE".to_owned()].into(),
            ..HomeLayout::default()
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
            ..HomeLayout::default()
        };
        // The day's feed does not have GONE; the user moves C up and shows A again.
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
        assert!(!l.featured_hidden);
        let l: HomeLayout = serde_json::from_str(r#"{"featuredHidden":true}"#).unwrap();
        assert!(l.featured_hidden);
        assert_eq!(serde_json::to_value(&l).unwrap()["featuredHidden"], true);
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
