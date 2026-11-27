//! Credits of a track (#84): GET v1 `tracks/{id}/credits?includeContributors=true`.
//! Checked on 2026-10-31 (`cargo test live_track_credits -- --ignored --nocapture`).
//!
//! Response: `[{type, contributors: [{name, id?}]}]`, `type` in free English text
//! (`Composer`, `Lyricist` or `Lyrics`, `Vocal` or `Vocals`, `OtherInstrument`…). `id` is
//! an artist id; `includeContributors=true` fills in more of them (engineers,
//! producers). A track without credits, or a nonexistent one, returns `[]` (no 404).
//!
//! Whole album (#206): GET v1 `albums/{id}/items/credits?includeContributors=true`,
//! paged (`totalNumberOfItems`, `limit` respected), items `{credits, item, type}` where
//! `credits` has the shape above and `item` is the full track (checked on 2026-11-05,
//! `cargo test live_album_credits -- --ignored --nocapture`).

use serde::Deserialize;

use super::Result;
use super::api::TidalClient;
use super::catalog::PAGE_SIZE;
use super::models::{AlbumContributor, AlbumCreditGroup, Contributor, CreditGroup, TrackId};

/// Tracks of an album read at most for its credits.
const ALBUM_MAX_ITEMS: usize = 1_000;

#[derive(Deserialize)]
struct RawCredit {
    #[serde(rename = "type", default)]
    role: String,
    #[serde(default)]
    contributors: Vec<RawContributor>,
}

#[derive(Deserialize)]
struct RawAlbumItem {
    #[serde(default)]
    credits: Vec<RawCredit>,
    item: Option<RawItemId>,
    #[serde(rename = "type", default)]
    kind: String,
}

#[derive(Deserialize)]
struct RawItemId {
    id: u64,
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

    pub async fn album_credits(&self, id: u64) -> Result<Vec<AlbumCreditGroup>> {
        let raw: Vec<RawAlbumItem> = self
            .get_all(
                &format!("albums/{id}/items/credits"),
                &[("includeContributors", "true".to_owned())],
                PAGE_SIZE,
                ALBUM_MAX_ITEMS,
            )
            .await?;
        Ok(album_groups(raw))
    }
}

/// Credits of each track (cleaned by `groups`) gathered by role and by person, with
/// the tracks concerned; a person present on every track lists none.
fn album_groups(raw: Vec<RawAlbumItem>) -> Vec<AlbumCreditGroup> {
    let items: Vec<(TrackId, Vec<CreditGroup>)> = raw
        .into_iter()
        .filter(|i| i.kind.is_empty() || i.kind == "track")
        .filter_map(|i| Some((TrackId(i.item?.id), groups(i.credits))))
        .collect();
    let mut out: Vec<AlbumCreditGroup> = Vec::new();
    for (track, credits) in &items {
        for credit in credits {
            let index = match out
                .iter()
                .position(|g| g.role.eq_ignore_ascii_case(&credit.role))
            {
                Some(i) => i,
                None => {
                    out.push(AlbumCreditGroup {
                        role: credit.role.clone(),
                        contributors: Vec::new(),
                    });
                    out.len() - 1
                }
            };
            let group = &mut out[index];
            for person in &credit.contributors {
                match group
                    .contributors
                    .iter_mut()
                    .find(|x| x.name == person.name)
                {
                    Some(existing) => {
                        existing.artist_id = existing.artist_id.or(person.artist_id);
                        if !existing.tracks.contains(track) {
                            existing.tracks.push(*track);
                        }
                    }
                    None => group.contributors.push(AlbumContributor {
                        name: person.name.clone(),
                        artist_id: person.artist_id,
                        tracks: vec![*track],
                    }),
                }
            }
        }
    }
    for group in &mut out {
        for person in &mut group.contributors {
            if person.tracks.len() == items.len() {
                person.tracks.clear();
            }
        }
    }
    out.sort_by_key(|g| rank(&g.role));
    out
}

/// Names cleaned, duplicates and empty groups removed, identical roles merged, then
/// groups ordered: writing, performers, production, technical, publishing.
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
    // Stable sort: the API order is kept within each category.
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
        // Performers and instruments.
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

    fn album(json: &str) -> Vec<AlbumCreditGroup> {
        album_groups(serde_json::from_str(json).unwrap())
    }

    #[test]
    fn album_credits_list_tracks_unless_everywhere() {
        let credits = album(
            r#"[{"type":"track","item":{"id":1},"credits":[
                    {"type":"Producer","contributors":[{"name":"Pat"}]},
                    {"type":"Drums","contributors":[{"name":"Dee","id":7}]}]},
                {"type":"track","item":{"id":2},"credits":[
                    {"type":"producer","contributors":[{"name":"Pat"}]},
                    {"type":"Composer","contributors":[{"name":"Cy"}]}]},
                {"type":"video","item":{"id":9},"credits":[
                    {"type":"Director","contributors":[{"name":"Vi"}]}]},
                {"type":"track","item":{"id":3},"credits":[
                    {"type":"Producer","contributors":[{"name":"Pat"}]},
                    {"type":"Drums","contributors":[{"name":"Dee"},{"name":"Ed"}]}]}]"#,
        );
        let roles: Vec<&str> = credits.iter().map(|g| g.role.as_str()).collect();
        assert_eq!(roles, ["Composer", "Drums", "Producer"]);
        assert_eq!(credits[0].contributors[0].tracks, [TrackId(2)]);
        let drums = &credits[1].contributors;
        assert_eq!(drums[0].artist_id, Some(7));
        assert_eq!(drums[0].tracks, [TrackId(1), TrackId(3)]);
        assert_eq!(drums[1].tracks, [TrackId(3)]);
        // On every track: no list.
        assert!(credits[2].contributors[0].tracks.is_empty());
    }

    #[test]
    fn empty_response() {
        assert!(album("[]").is_empty());
        assert!(parse("[]").is_empty());
    }
}
