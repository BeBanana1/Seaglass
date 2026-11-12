//! Bas de la page d'album (#350) : autres versions du même album et autres albums de l'artiste.
//!
//! Endpoint vérifié le 2026-11-12 (`cargo test live_album_related -- --ignored --nocapture`) :
//! `pages/album?albumId=&deviceType=BROWSER&locale=` → `{ rows: [{ modules: [...] }] }`, avec
//! `ALBUM_HEADER`, `ALBUM_ITEMS`, puis des `ALBUM_LIST` (`pagedList.items` = albums v1) et un
//! `ARTIST_LIST`. Les titres des modules sont traduits ; chaque module se reconnaît à un UUID
//! stable d'un album à l'autre, présent dans `id` (JSON en base64, champ `m`) et dans
//! `showMore.apiPath`. Un album absent du pays répond 404.
//!
//! « Autres versions » ne donne pas toujours le jumeau hi-res au même titre (`twin.rs`) :
//! la recherche de #55 / #59 reste nécessaire, l'UI retire seulement ce jumeau de la rangée.

use base64::Engine as _;
use serde::Deserialize;

use super::api::TidalClient;
use super::models::{AlbumRelated, AlbumSummary};
use super::raw::RawAlbumFull;
use super::{Result, TidalError};

/// Module « Autres versions ».
const VERSIONS_MODULE: &str = "7f62f3ef-6499-4dc8-9229-148d5f3b3b2e";
/// Module « Plus d'albums par <artiste> ».
const BY_ARTIST_MODULE: &str = "3f410242-777b-43b6-9acf-6ed91cb95327";

impl TidalClient {
    /// Rangées du bas de la page d'album. Jamais d'erreur : une page absente (404) ou illisible
    /// donne des listes vides (journalisées), la page d'album reste affichée sans elles.
    pub async fn album_related(&self, id: u64) -> AlbumRelated {
        let params = [
            ("albumId", id.to_string()),
            ("deviceType", "BROWSER".to_owned()),
            ("locale", crate::i18n::tidal_locale().to_owned()),
        ];
        let page: Result<RawAlbumPage> = self.get("pages/album", &params).await;
        match page {
            Ok(page) => page.related(id),
            Err(TidalError::Api { status: 404, .. }) => AlbumRelated::default(),
            Err(e) => {
                tracing::warn!("pages/album {id} : {e}");
                AlbumRelated::default()
            }
        }
    }
}

#[derive(Deserialize)]
struct RawAlbumPage {
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
    #[serde(default)]
    id: String,
    show_more: Option<RawShowMore>,
    paged_list: Option<RawPagedList>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawShowMore {
    #[serde(default)]
    api_path: String,
}

#[derive(Deserialize)]
struct RawPagedList {
    /// Lus un par un : un album d'une forme inattendue est omis sans perdre la rangée.
    #[serde(default)]
    items: Vec<serde_json::Value>,
}

impl RawModule {
    /// Le module porte cet UUID (dans `showMore.apiPath` ou dans son `id` décodé).
    fn is(&self, uuid: &str) -> bool {
        if self
            .show_more
            .as_ref()
            .is_some_and(|m| m.api_path.contains(uuid))
        {
            return true;
        }
        base64::engine::general_purpose::STANDARD
            .decode(&self.id)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .is_some_and(|decoded| decoded.contains(uuid))
    }

    fn albums(self, exclude: u64) -> Vec<AlbumSummary> {
        let items = self.paged_list.map(|l| l.items).unwrap_or_default();
        let mut out: Vec<AlbumSummary> = Vec::with_capacity(items.len());
        for item in items {
            let Ok(raw) = serde_json::from_value::<RawAlbumFull>(item) else {
                continue;
            };
            if raw.id != exclude && !out.iter().any(|a| a.id == raw.id) {
                out.push(raw.into());
            }
        }
        out
    }
}

impl RawAlbumPage {
    /// Garde les deux modules attendus ; l'album affiché en est retiré.
    fn related(self, album_id: u64) -> AlbumRelated {
        let mut related = AlbumRelated::default();
        for module in self.rows.into_iter().flat_map(|r| r.modules) {
            if module.kind != "ALBUM_LIST" {
                continue;
            }
            if related.versions.is_empty() && module.is(VERSIONS_MODULE) {
                related.versions = module.albums(album_id);
            } else if related.by_artist.is_empty() && module.is(BY_ARTIST_MODULE) {
                related.by_artist = module.albums(album_id);
            }
        }
        related
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn album(id: u64, title: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id, "title": title, "cover": "ab-cd", "artists": [{"id": 1, "name": "A"}],
            "explicit": false, "ai": null, "releaseDate": "2021-09-24",
            "mediaMetadata": {"tags": ["LOSSLESS", "HIRES_LOSSLESS"]}
        })
    }

    fn page() -> RawAlbumPage {
        let encode = |m: &str| {
            base64::engine::general_purpose::STANDARD
                .encode(format!(r#"{{"p":"page","pV":13,"m":"{m}","mV":1}}"#))
        };
        serde_json::from_value(serde_json::json!({
            "rows": [
                {"modules": [{"type": "ALBUM_HEADER", "id": "x", "album": {}}]},
                {"modules": [{"type": "ALBUM_ITEMS", "pagedList": {"items": [{"type": "track"}]}}]},
                // Reconnu par son `id` seul (pas de `showMore`).
                {"modules": [{
                    "type": "ALBUM_LIST", "title": " Autres Versions", "id": encode(VERSIONS_MODULE),
                    "pagedList": {"items": [album(2, "Deluxe"), album(10, "Courant"), {"id": "illisible"}, album(3, "Remaster"), album(2, "Deluxe")]}
                }]},
                {"modules": [{
                    "type": "ALBUM_LIST", "title": "Plus d'Albums par A", "id": "pas du base64",
                    "showMore": {"apiPath": format!("pages/single-module-page/p/13/{BY_ARTIST_MODULE}/1?albumId=10&artistId=1")},
                    "pagedList": {"items": [album(4, "Autre")]}
                }]},
                {"modules": [{
                    "type": "ALBUM_LIST", "title": "Albums similaires", "id": encode("d788041e-3449-4d21-8137-4ae59a70cb3e"),
                    "pagedList": {"items": [album(5, "Similaire")]}
                }]},
                {"modules": [{"type": "ARTIST_LIST", "id": encode(BY_ARTIST_MODULE), "pagedList": {"items": []}}]}
            ]
        }))
        .unwrap()
    }

    #[test]
    fn keeps_versions_and_artist_albums() {
        let related = page().related(10);
        let ids = |albums: &[AlbumSummary]| albums.iter().map(|a| a.id).collect::<Vec<_>>();
        // Album courant, élément illisible et doublon retirés ; ordre de l'API.
        assert_eq!(ids(&related.versions), [2, 3]);
        assert_eq!(ids(&related.by_artist), [4]);
        assert!(related.versions[0].hires);
    }

    #[test]
    fn unknown_or_empty_page_gives_nothing() {
        let empty: RawAlbumPage = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(empty.related(1), AlbumRelated::default());
        let other: RawAlbumPage = serde_json::from_value(serde_json::json!({
            "rows": [{"modules": [{"type": "ALBUM_LIST", "id": "", "pagedList": {"items": [album(5, "X")]}}]}]
        }))
        .unwrap();
        assert_eq!(other.related(1), AlbumRelated::default());
    }
}
