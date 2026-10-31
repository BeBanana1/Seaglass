//! Crédits d'un titre (#84) : GET v1 `tracks/{id}/credits?includeContributors=true`.
//! Vérifié le 2026-10-31 (`cargo test live_track_credits -- --ignored --nocapture`).
//!
//! Réponse : `[{type, contributors: [{name, id?}]}]`, `type` en texte libre anglais
//! (`Composer`, `Lyricist` ou `Lyrics`, `Vocal` ou `Vocals`, `OtherInstrument`…). `id` est
//! un identifiant d'artiste ; `includeContributors=true` en renseigne davantage (ingénieurs,
//! producteurs). Un titre sans crédits, ou inexistant, renvoie `[]` (pas de 404).

use serde::Deserialize;

use super::Result;
use super::api::TidalClient;
use super::models::{Contributor, CreditGroup, TrackId};

#[derive(Deserialize)]
struct RawCredit {
    #[serde(rename = "type", default)]
    role: String,
    #[serde(default)]
    contributors: Vec<RawContributor>,
}

#[derive(Deserialize)]
struct RawContributor {
    #[serde(default)]
    name: String,
    id: Option<u64>,
}

impl TidalClient {
    pub async fn track_credits(&self, id: TrackId) -> Result<Vec<CreditGroup>> {
        let raw: Vec<RawCredit> = self
            .get(
                &format!("tracks/{id}/credits"),
                &[("includeContributors", "true".to_owned())],
            )
            .await?;
        Ok(groups(raw))
    }
}

/// Noms nettoyés, doublons et groupes vides retirés, rôles identiques fusionnés, puis
/// groupes rangés : écriture, interprètes, production, technique, édition.
fn groups(raw: Vec<RawCredit>) -> Vec<CreditGroup> {
    let mut out: Vec<CreditGroup> = Vec::new();
    for credit in raw {
        let role = credit.role.trim();
        if role.is_empty() {
            continue;
        }
        let index = match out.iter().position(|g| g.role.eq_ignore_ascii_case(role)) {
            Some(i) => i,
            None => {
                out.push(CreditGroup {
                    role: role.to_owned(),
                    contributors: Vec::new(),
                });
                out.len() - 1
            }
        };
        let group = &mut out[index];
        for c in credit.contributors {
            let name = c.name.trim();
            if name.is_empty() {
                continue;
            }
            match group.contributors.iter_mut().find(|x| x.name == name) {
                Some(existing) => existing.artist_id = existing.artist_id.or(c.id),
                None => group.contributors.push(Contributor {
                    name: name.to_owned(),
                    artist_id: c.id,
                }),
            }
        }
    }
    out.retain(|g| !g.contributors.is_empty());
    // Tri stable : l'ordre de l'API est gardé dans chaque catégorie.
    out.sort_by_key(|g| rank(&g.role));
    out
}

fn rank(role: &str) -> u8 {
    let role = role.to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| role.contains(w));
    if has(&["composer", "lyric", "writer", "author", "librettist"]) {
        0
    } else if has(&["publisher", "label", "copyright"]) {
        5
    } else if has(&[
        "engineer",
        "mastering",
        "mixing",
        "mixer",
        "editing",
        "recording",
    ]) {
        4
    } else if has(&["producer", "arranger", "a&r"]) {
        3
    } else {
        // Interprètes et instruments.
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Vec<CreditGroup> {
        groups(serde_json::from_str(json).unwrap())
    }

    #[test]
    fn groups_are_ranked_and_cleaned() {
        let credits = parse(
            r#"[{"contributors":[{"id":8992,"name":"Queen"}],"type":"Producer"},
                {"contributors":[{"id":9664,"name":"Freddie Mercury"}],"type":"Composer"},
                {"contributors":[{"name":"Kris Fredriksson"}],"type":"Mixing Engineer"},
                {"contributors":[{"id":3513781,"name":"Roger Taylor"}],"type":"Drums"},
                {"contributors":[],"type":"Piano"},
                {"contributors":[{"name":"Label Controlled"}],"type":"Music Publisher"},
                {"contributors":[{"name":" "}],"type":"Vocal"}]"#,
        );
        let roles: Vec<&str> = credits.iter().map(|g| g.role.as_str()).collect();
        assert_eq!(
            roles,
            [
                "Composer",
                "Drums",
                "Producer",
                "Mixing Engineer",
                "Music Publisher"
            ]
        );
        assert_eq!(credits[0].contributors[0].artist_id, Some(9664));
        assert_eq!(credits[3].contributors[0].artist_id, None);
    }

    #[test]
    fn same_role_is_merged_without_duplicates() {
        let credits = parse(
            r#"[{"contributors":[{"name":"Ann"}],"type":"Engineer"},
                {"contributors":[{"name":"Ann","id":5},{"name":"Bob"}],"type":"engineer"}]"#,
        );
        assert_eq!(credits.len(), 1);
        assert_eq!(
            credits[0].contributors,
            [
                Contributor {
                    name: "Ann".into(),
                    artist_id: Some(5)
                },
                Contributor {
                    name: "Bob".into(),
                    artist_id: None
                }
            ]
        );
    }

    #[test]
    fn empty_response() {
        assert!(parse("[]").is_empty());
    }
}
