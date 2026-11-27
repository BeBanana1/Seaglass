//! Page of a contributor (#429): tracks a person contributed to, with their roles.
//!
//! Endpoints checked on 2026-11-18 (`cargo test live_contributor -- --ignored --nocapture`):
//! - `pages/contributor?artistId=&deviceType=BROWSER&locale=` → `{ rows: [{ modules: [...] }] }`
//!   with `CONTRIBUTOR_HEADER` (`artist`: id, name, photo) then `ITEM_LIST_WITH_ROLES`, whose
//!   `pagedList` carries 50 items `{type: "track", item: <v1 track>, roles: [{category,
//!   categoryId, name}]}`, `totalNumberOfItems` (7,605 for Hans Zimmer) and `dataApiPath`
//!   (`pages/data/<uuid>?artistId=`). Unknown id: 404.
//! - Next pages: `dataApiPath` + `offset` / `limit` → `{items, totalNumberOfItems}`; `limit` of
//!   50 at most (400 beyond). Read to the end: past the total, empty list.
//! - No server-side role filter (`roleCategoryId`, `category`… ignored).
//!
//! The id is the artist's: the credits (`includeContributors=true`) give it.

use serde::Deserialize;

use super::api::TidalClient;
use super::models::{ArtistSummary, Contribution, ContributorPage};
use super::paging::try_join_bounded;
use super::raw::{RawArtistFull, RawTrack};
use super::{Result, TidalError};

/// Items per request: the maximum the API accepts.
const PAGE_SIZE: u32 = 50;
/// Items read per call (the page, then each « More tracks »).
pub const BATCH: u32 = 200;
/// Page requests in flight at most.
const CONCURRENCY: usize = 4;

impl TidalClient {
    /// `BATCH` items from `offset` (0: header and first tracks). The page is
    /// always read again: it alone gives the path of the following pages.
    pub async fn contributor(&self, id: u64, offset: u32) -> Result<ContributorPage> {
        let params = [
            ("artistId", id.to_string()),
            ("deviceType", "BROWSER".to_owned()),
            ("locale", crate::i18n::tidal_locale().to_owned()),
        ];
        let page: RawContributorPage = self.get("pages/contributor", &params).await?;
        let parsed = page.parse().ok_or_else(|| {
            TidalError::Decode(format!("pages/contributor {id}: credits module missing"))
        })?;
        let total = parsed.total;
        let mut items: Vec<serde_json::Value> = Vec::new();
        let mut start = offset;
        if offset == 0 {
            start = parsed.items.len() as u32;
            items = parsed.items;
        }
        let offsets = data_offsets(start, offset.saturating_add(BATCH), total);
        // Next: end of the window read, in API offsets (not in tracks kept: the
        // content filter and unreadable tracks remove some).
        let mut next = None;
        match (parsed.data, offsets.last()) {
            (_, None) => next = Some(start),
            (None, Some(_)) => {}
            (Some(data), Some(&last)) => {
                let mut query: Vec<(&str, String)> = data.query.clone();
                query.push(("deviceType", "BROWSER".to_owned()));
                query.push(("locale", crate::i18n::tidal_locale().to_owned()));
                let mut requests = Vec::with_capacity(offsets.len());
                for &o in &offsets {
                    requests.push(self.data_page(&data.path, &query, o));
                }
                let mut exhausted = false;
                for page in try_join_bounded(requests, CONCURRENCY).await? {
                    exhausted |= page.items.is_empty();
                    items.extend(page.items);
                }
                if !exhausted {
                    next = Some(last + PAGE_SIZE);
                }
            }
        }
        Ok(ContributorPage {
            artist: parsed.artist,
            total,
            next_offset: next.filter(|&n| n > offset && n < total),
            tracks: contributions(items),
        })
    }

    async fn data_page(
        &self,
        path: &str,
        query: &[(&'static str, String)],
        offset: u32,
    ) -> Result<RawData> {
        let mut params: Vec<(&str, String)> = query.to_vec();
        params.push(("offset", offset.to_string()));
        params.push(("limit", PAGE_SIZE.to_string()));
        self.get(path, &params).await
    }
}

/// Offsets of the data pages to read, from `start` up to `end` (excluded), within `total`.
fn data_offsets(start: u32, end: u32, total: u32) -> Vec<u32> {
    (start..end.min(total))
        .step_by(PAGE_SIZE as usize)
        .collect()
}

/// Playable tracks with their roles; items of an unexpected shape skipped one by one.
fn contributions(items: Vec<serde_json::Value>) -> Vec<Contribution> {
    let mut out: Vec<Contribution> = Vec::with_capacity(items.len());
    for item in items {
        let Ok(raw) = serde_json::from_value::<RawContribution>(item) else {
            continue;
        };
        if raw.kind != "track" {
            continue;
        }
        let Some(track) = raw.item.into_track() else {
            continue;
        };
        if out.iter().any(|c| c.track.id == track.id) {
            continue;
        }
        let mut roles: Vec<String> = Vec::new();
        for role in raw.roles {
            let name = role.name.trim();
            if !name.is_empty() && !roles.iter().any(|r| r.eq_ignore_ascii_case(name)) {
                roles.push(name.to_owned());
            }
        }
        out.push(Contribution { track, roles });
    }
    out
}

#[derive(Deserialize)]
struct RawContributorPage {
    #[serde(default)]
    rows: Vec<RawRow>,
}

#[derive(Deserialize)]
struct RawRow {
    #[serde(default)]
    modules: Vec<RawModule>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawModule {
    #[serde(rename = "type", default)]
    kind: String,
    artist: Option<RawArtistFull>,
    paged_list: Option<RawPagedList>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPagedList {
    /// Read one by one: an item of an unexpected shape is skipped without losing the page.
    #[serde(default)]
    items: Vec<serde_json::Value>,
    total_number_of_items: Option<u32>,
    data_api_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawData {
    #[serde(default)]
    items: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct RawContribution {
    #[serde(rename = "type", default)]
    kind: String,
    item: RawTrack,
    #[serde(default)]
    roles: Vec<RawRole>,
}

#[derive(Deserialize)]
struct RawRole {
    #[serde(default)]
    name: String,
}

struct Parsed {
    artist: ArtistSummary,
    total: u32,
    items: Vec<serde_json::Value>,
    data: Option<DataPath>,
}

/// Path of the following pages, read again in the response (`pages/data/<uuid>?artistId=…`).
#[derive(Debug, PartialEq)]
struct DataPath {
    path: String,
    query: Vec<(&'static str, String)>,
}

impl DataPath {
    /// Only a `pages/data/…` path made of safe characters is followed; only `artistId` is
    /// kept from its query.
    fn parse(api_path: &str) -> Option<Self> {
        let (path, query) = api_path.split_once('?').unwrap_or((api_path, ""));
        let rest = path.strip_prefix("pages/data/")?;
        let safe = !rest.is_empty()
            && rest
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '/');
        if !safe || rest.contains("//") {
            return None;
        }
        let query = query
            .split('&')
            .filter_map(|kv| kv.split_once('='))
            .filter(|(k, v)| *k == "artistId" && v.chars().all(|c| c.is_ascii_digit()))
            .map(|(_, v)| ("artistId", v.to_owned()))
            .collect();
        Some(Self {
            path: path.to_owned(),
            query,
        })
    }
}

impl RawContributorPage {
    fn parse(self) -> Option<Parsed> {
        let mut artist: Option<ArtistSummary> = None;
        let mut list: Option<RawPagedList> = None;
        for module in self.rows.into_iter().flat_map(|r| r.modules) {
            match module.kind.as_str() {
                "CONTRIBUTOR_HEADER" if artist.is_none() => {
                    artist = module.artist.map(ArtistSummary::from);
                }
                "ITEM_LIST_WITH_ROLES" if list.is_none() => list = module.paged_list,
                _ => {}
            }
        }
        let list = list?;
        let total = list
            .total_number_of_items
            .unwrap_or(list.items.len() as u32);
        Some(Parsed {
            artist: artist?,
            total,
            data: list.data_api_path.as_deref().and_then(DataPath::parse),
            items: list.items,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::TrackId;

    fn track(id: u64, roles: &[&str]) -> serde_json::Value {
        serde_json::json!({
            "type": "track",
            "roles": roles.iter().map(|r| serde_json::json!({"category": "X", "categoryId": 1, "name": r})).collect::<Vec<_>>(),
            "item": {
                "id": id, "title": format!("T{id}"), "duration": 100, "explicit": false,
                "artists": [{"id": 1, "name": "A"}],
                "album": {"id": 9, "title": "Al", "cover": "ab-cd"}
            }
        })
    }

    fn page(items: Vec<serde_json::Value>, total: u32) -> RawContributorPage {
        serde_json::from_value(serde_json::json!({
            "rows": [
                {"modules": [{"type": "CONTRIBUTOR_HEADER", "title": "Credits for Z",
                    "artist": {"id": 12979, "name": "Hans Zimmer", "picture": "4a28-d738"}}]},
                {"modules": [{"type": "ITEM_LIST_WITH_ROLES", "pagedList": {
                    "items": items, "totalNumberOfItems": total, "limit": 50, "offset": 0,
                    "dataApiPath": "pages/data/58e37dd4-d8b9-447f-a39b-419c2f69408d?artistId=12979"
                }}]}
            ]
        }))
        .unwrap()
    }

    #[test]
    fn reads_header_list_and_data_path() {
        let parsed = page(vec![track(1, &["Producer"])], 7605).parse().unwrap();
        assert_eq!(parsed.artist.id, 12979);
        assert_eq!(parsed.artist.name, "Hans Zimmer");
        assert!(parsed.artist.picture_url.is_some());
        assert_eq!(parsed.total, 7605);
        assert_eq!(parsed.items.len(), 1);
        assert_eq!(
            parsed.data,
            Some(DataPath {
                path: "pages/data/58e37dd4-d8b9-447f-a39b-419c2f69408d".into(),
                query: vec![("artistId", "12979".into())],
            })
        );
    }

    #[test]
    fn missing_list_or_header_is_none() {
        let empty: RawContributorPage = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(empty.parse().is_none());
        let no_header: RawContributorPage = serde_json::from_value(serde_json::json!({
            "rows": [{"modules": [{"type": "ITEM_LIST_WITH_ROLES", "pagedList": {"items": []}}]}]
        }))
        .unwrap();
        assert!(no_header.parse().is_none());
    }

    #[test]
    fn only_safe_data_paths_are_followed() {
        assert!(DataPath::parse("pages/data/abc-123").is_some());
        assert!(DataPath::parse("https://evil/pages/data/x").is_none());
        assert!(DataPath::parse("pages/data/../sessions").is_none());
        assert!(DataPath::parse("pages/data/").is_none());
        assert!(DataPath::parse("tracks/1").is_none());
        // Parameters other than a numeric `artistId` ignored.
        let p = DataPath::parse("pages/data/x?artistId=1&token=a&artistId=b").unwrap();
        assert_eq!(p.query, vec![("artistId", "1".to_owned())]);
    }

    #[test]
    fn offsets_stop_at_batch_and_total() {
        // First read: 50 items already in the page, 3 more pages.
        assert_eq!(data_offsets(50, BATCH, 7605), [50, 100, 150]);
        assert_eq!(data_offsets(200, 400, 7605), [200, 250, 300, 350]);
        assert_eq!(data_offsets(50, BATCH, 120), [50, 100]);
        assert_eq!(data_offsets(50, BATCH, 40), Vec::<u32>::new());
        assert_eq!(data_offsets(7600, 7800, 7605), [7600]);
    }

    #[test]
    fn contributions_keep_tracks_with_deduplicated_roles() {
        let items = vec![
            track(1, &["Producer", "Composer", "producer", " "]),
            serde_json::json!({"type": "video", "item": {"id": 5}, "roles": []}),
            serde_json::json!({"type": "track", "item": {"id": "illisible"}}),
            // Without album: ignored as elsewhere.
            serde_json::json!({"type": "track", "roles": [], "item": {"id": 3, "title": "x", "duration": 1}}),
            track(2, &[]),
            track(1, &["Mixer"]),
        ];
        let out = contributions(items);
        let ids: Vec<TrackId> = out.iter().map(|c| c.track.id).collect();
        assert_eq!(ids, [TrackId(1), TrackId(2)]);
        assert_eq!(out[0].roles, ["Producer", "Composer"]);
        assert!(out[1].roles.is_empty());
    }
}
