//! Mixes et radios.
//!
//! Endpoints vérifiés (2026-10-30) : `pages/mix?mixId=` (en-tête du mix),
//! `mixes/{id}/items` (`{ item, type: "track" }` paginé), v2 `favorites/mixes` (paginé par
//! curseur), `tracks/{id}/radio` et `artists/{id}/radio` (pages de titres). Un même mix
//! arrive sous trois formes selon l'endpoint : voir `RawMix`.
//! Titres et artistes bloqués retirés des mixes et des radios (#334, `blocks.rs`).

use std::collections::HashMap;

use serde::Deserialize;

use super::api::{API_V2, TidalClient};
use super::likes::LikedKind;
use super::models::{MixPage, MixSummary, RadioPage, RadioSeed, TrackId};
use super::raw::{self, RawArtistFull, RawPage, RawTrack, image_url};
use super::{Result, TidalError};

/// Taille de page de `mixes/{id}/items` (un mix compte rarement plus de 100 titres).
const MIX_PAGE_SIZE: u32 = 100;
const MIX_MAX_TRACKS: usize = 500;
/// `favorites/mixes` (v2) : 50 par page, comme les playlists.
const FAVORITE_MIXES_PAGE: u32 = 50;
const FAVORITE_MIXES_MAX: usize = 500;
/// L'API renvoie au plus 100 titres de radio (96 vus pour un titre, 100 pour un artiste).
const RADIO_SIZE: u32 = 100;

impl TidalClient {
    /// En-tête et titres d'un mix (deux requêtes en parallèle).
    pub async fn mix(&self, id: &str) -> Result<MixPage> {
        let page_params = [
            ("mixId", id.to_owned()),
            ("deviceType", "BROWSER".to_owned()),
            ("locale", crate::i18n::tidal_locale().to_owned()),
        ];
        let items_path = format!("mixes/{id}/items");
        let (page, items) = tokio::try_join!(
            self.get::<RawMixPage>("pages/mix", &page_params),
            self.get_all::<RawMixItem>(&items_path, &[], MIX_PAGE_SIZE, MIX_MAX_TRACKS),
        )?;
        let raw = page
            .header()
            .ok_or_else(|| TidalError::Decode(format!("mix {id} sans en-tête")))?;
        let image_large_url = raw.image("MEDIUM");
        let mix = raw
            .into_summary()
            .ok_or_else(|| TidalError::Decode(format!("mix {id} sans titre")))?;
        Ok(MixPage {
            mix,
            image_large_url,
            tracks: self
                .without_blocked(self.remember(raw::tracks(track_items(items))))
                .await,
        })
    }

    /// Mixes enregistrés par l'utilisateur (My Mix, Daily Discovery…), dans l'ordre de l'API.
    pub async fn favorite_mixes(&self) -> Result<Vec<MixSummary>> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut query = vec![("limit", FAVORITE_MIXES_PAGE.to_string())];
            if let Some(c) = &cursor {
                query.push(("cursor", c.clone()));
            }
            let page: RawCursorPage = self.get_at(API_V2, "favorites/mixes", &query).await?;
            out.extend(page.items.into_iter().filter_map(RawMix::into_summary));
            cursor = page.cursor.filter(|c| !c.is_empty());
            if cursor.is_none() || out.len() >= FAVORITE_MIXES_MAX {
                break;
            }
        }
        if out.len() < FAVORITE_MIXES_MAX {
            // Liste complète : les cœurs des mixes suivent (enregistrés ailleurs).
            let ids = out.iter().map(|m| m.id.clone()).collect();
            self.likes.replace(LikedKind::Mixes(ids));
        }
        out.truncate(FAVORITE_MIXES_MAX);
        Ok(out)
    }

    /// Identifiants des mixes enregistrés (#204) : v2 `favorites/mixes/ids`, par curseur
    /// (`{content: ["<id>", …], cursor}`), triés par id. `favorites/ids` ne les a pas.
    pub(super) async fn favorite_mix_ids(&self) -> Result<Vec<String>> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut query = vec![("limit", FAVORITE_MIXES_PAGE.to_string())];
            if let Some(c) = &cursor {
                query.push(("cursor", c.clone()));
            }
            let page: RawIdPage = self.get_at(API_V2, "favorites/mixes/ids", &query).await?;
            out.extend(page.content);
            cursor = page.cursor.filter(|c| !c.is_empty());
            if cursor.is_none() || out.len() >= FAVORITE_MIXES_MAX {
                break;
            }
        }
        Ok(out)
    }

    /// Radio d'un titre ou d'un artiste, avec de quoi afficher son point de départ.
    pub async fn radio(&self, seed: RadioSeed, id: u64) -> Result<RadioPage> {
        let params = [("limit", RADIO_SIZE.to_string())];
        let (title, image_url, tracks) = match seed {
            RadioSeed::Track => {
                let path = format!("tracks/{id}/radio");
                let (track, page) = tokio::try_join!(
                    self.track(TrackId(id)),
                    self.get::<RawPage<RawTrack>>(&path, &params),
                )?;
                (track.title, track.album.cover_url, page.items)
            }
            RadioSeed::Artist => {
                let (path, radio_path) = (format!("artists/{id}"), format!("artists/{id}/radio"));
                let (artist, page) = tokio::try_join!(
                    self.get::<RawArtistFull>(&path, &[]),
                    self.get::<RawPage<RawTrack>>(&radio_path, &params),
                )?;
                let picture = artist.picture.as_deref().map(|p| image_url(p, 320));
                (artist.name, picture, page.items)
            }
        };
        Ok(RadioPage {
            seed,
            id,
            title,
            image_url,
            tracks: self
                .without_blocked(self.remember(raw::tracks(tracks)))
                .await,
        })
    }
}

// --- Formats JSON bruts --------------------------------------------------------------

/// Mix sous ses trois formes :
/// - accueil v2 : `titleTextInfo.text`, `subtitleTextInfo.text`, `mixImages: [{size, url}]`, `type` ;
/// - `favorites/mixes` : `title`, `subTitle`, `images: {SMALL: {url}}`, `mixType` ;
/// - `pages/mix` : comme les favoris.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawMix {
    id: String,
    title: Option<String>,
    title_text_info: Option<RawText>,
    sub_title: Option<String>,
    #[serde(alias = "subTitleTextInfo")]
    subtitle_text_info: Option<RawText>,
    images: Option<HashMap<String, RawImage>>,
    mix_images: Option<Vec<RawSizedImage>>,
    mix_type: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
}

#[derive(Deserialize)]
struct RawText {
    text: Option<String>,
}

#[derive(Deserialize)]
struct RawImage {
    url: String,
}

#[derive(Deserialize)]
struct RawSizedImage {
    size: String,
    url: String,
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

impl RawMix {
    /// URL de l'image `SMALL` (320), `MEDIUM` (640) ou `LARGE` (1500). Les URLs sont signées
    /// par l'API : elles sont gardées telles quelles.
    fn image(&self, size: &str) -> Option<String> {
        let from_map = self
            .images
            .as_ref()
            .and_then(|m| m.get(size))
            .map(|i| i.url.clone());
        from_map.or_else(|| {
            self.mix_images
                .iter()
                .flatten()
                .find(|i| i.size == size)
                .map(|i| i.url.clone())
        })
    }

    /// `None` si le mix n'a pas de titre (rien d'affichable).
    pub(super) fn into_summary(self) -> Option<MixSummary> {
        let image_url = self.image("SMALL");
        let title = non_empty(self.title).or_else(|| non_empty(self.title_text_info?.text))?;
        let subtitle =
            non_empty(self.sub_title).or_else(|| non_empty(self.subtitle_text_info?.text));
        Some(MixSummary {
            id: self.id,
            title,
            subtitle,
            image_url,
            mix_type: self.mix_type.or(self.kind),
        })
    }
}

/// `pages/mix` : `{ rows: [{ modules: [{ type: "MIX_HEADER", mix }, { type: "TRACK_LIST", … }] }] }`.
#[derive(Deserialize)]
struct RawMixPage {
    #[serde(default)]
    rows: Vec<RawPageRow>,
}

#[derive(Deserialize)]
struct RawPageRow {
    #[serde(default)]
    modules: Vec<RawPageModule>,
}

#[derive(Deserialize)]
struct RawPageModule {
    /// Lu à part : un autre module pourrait porter un champ `mix` d'une autre forme.
    #[serde(default)]
    mix: serde_json::Value,
}

impl RawMixPage {
    fn header(self) -> Option<RawMix> {
        self.rows
            .into_iter()
            .flat_map(|r| r.modules)
            .find_map(|m| serde_json::from_value(m.mix).ok())
    }
}

/// Élément de `mixes/{id}/items` : seuls les `track` sont gardés (un mix peut contenir des vidéos).
#[derive(Deserialize)]
struct RawMixItem {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    item: serde_json::Value,
}

fn track_items(items: Vec<RawMixItem>) -> Vec<RawTrack> {
    items
        .into_iter()
        .filter(|i| i.kind == "track")
        .filter_map(|i| serde_json::from_value(i.item).ok())
        .collect()
}

#[derive(Deserialize)]
struct RawIdPage {
    #[serde(default)]
    content: Vec<String>,
    cursor: Option<String>,
}

#[derive(Deserialize)]
struct RawCursorPage {
    #[serde(default)]
    items: Vec<RawMix>,
    cursor: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(value: serde_json::Value) -> MixSummary {
        serde_json::from_value::<RawMix>(value)
            .unwrap()
            .into_summary()
            .unwrap()
    }

    #[test]
    fn home_mix_shape() {
        let mix = parse(json!({
            "id": "01636e", "type": "DISCOVERY_MIX",
            "titleTextInfo": {"color": "#FBD293", "text": "My Daily Discovery"},
            "subtitleTextInfo": {"color": "#FBD293", "text": "Réactualisation quotidienne."},
            "mixImages": [{"size": "SMALL", "url": "https://i/s"}, {"size": "MEDIUM", "url": "https://i/m"}]
        }));
        assert_eq!(mix.title, "My Daily Discovery");
        assert_eq!(
            mix.subtitle.as_deref(),
            Some("Réactualisation quotidienne.")
        );
        assert_eq!(mix.image_url.as_deref(), Some("https://i/s"));
        assert_eq!(mix.mix_type.as_deref(), Some("DISCOVERY_MIX"));
    }

    #[test]
    fn favorite_mix_shape() {
        let mix = parse(json!({
            "id": "0027af", "mixType": "DAILY_MIX", "title": "My Mix 1",
            "subTitle": "Limp Bizkit, Pierce The Veil and more",
            "subTitleTextInfo": {"text": "ignoré"},
            "images": {"SMALL": {"url": "https://i/s", "width": 320}, "LARGE": {"url": "https://i/l"}}
        }));
        assert_eq!(mix.title, "My Mix 1");
        assert_eq!(
            mix.subtitle.as_deref(),
            Some("Limp Bizkit, Pierce The Veil and more")
        );
        assert_eq!(mix.image_url.as_deref(), Some("https://i/s"));
        assert_eq!(mix.mix_type.as_deref(), Some("DAILY_MIX"));
    }

    #[test]
    fn empty_texts_are_dropped() {
        let raw: RawMix = serde_json::from_value(json!({
            "id": "m", "title": " ", "titleTextInfo": {"text": null}
        }))
        .unwrap();
        assert_eq!(raw.into_summary(), None);
        let mix = parse(
            json!({"id": "m", "title": "t", "subTitle": "", "subtitleTextInfo": {"text": null}}),
        );
        assert_eq!(mix.subtitle, None);
        assert_eq!(mix.image_url, None);
    }

    #[test]
    fn page_header_is_found() {
        let page: RawMixPage = serde_json::from_value(json!({
            "rows": [
                {"modules": [{"type": "MIX_HEADER", "mix": {"id": "m", "title": "t",
                    "images": {"MEDIUM": {"url": "https://i/m"}}}}]},
                {"modules": [{"type": "TRACK_LIST", "pagedList": {}}]}
            ]
        }))
        .unwrap();
        let header = page.header().unwrap();
        assert_eq!(header.image("MEDIUM").as_deref(), Some("https://i/m"));
        assert_eq!(header.into_summary().unwrap().title, "t");
    }

    #[test]
    fn only_track_items_are_kept() {
        let items: Vec<RawMixItem> = serde_json::from_value(json!([
            {"type": "track", "item": {"id": 1, "title": "a", "duration": 10, "album": {"id": 3, "title": "c"}}},
            {"type": "video", "item": {"id": 2, "title": "clip"}},
            {"type": "track", "item": {"title": "sans id"}}
        ]))
        .unwrap();
        let tracks = raw::tracks(track_items(items));
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, TrackId(1));
    }

    /// `MixSummary` ne doit pas avoir de champ `kind` (étiquette de `MediaItem`).
    #[test]
    fn mix_item_tag_is_unique() {
        let item = super::super::models::MediaItem::Mix(parse(
            json!({"id": "m", "title": "t", "type": "DAILY_MIX"}),
        ));
        let value = serde_json::to_value(&item).unwrap();
        assert_eq!(value["kind"], "mix");
        assert_eq!(value["mixType"], "DAILY_MIX");
    }
}
