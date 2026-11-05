//! Accueil personnalisé : v2 `home/feed/static?deviceType=BROWSER&platform=WEB&locale=…`,
//! lignes `{moduleId, title, items: [{type, data}]}`.
//!
//! Le flux est paginé (#207, vérifié le 2026-11-05) : `page.cursor` de la réponse, repassé
//! en `cursor=` avec les mêmes paramètres, donne la page suivante ; `null` sur la dernière.
//! Deux pages vues (11 puis 7 lignes), `moduleId` tous différents, ~0,5 s chacune. La
//! première page est rendue seule (`home_start`), puis la suite est lue à part
//! (`home_rest`), qui repart de la première page gardée ici : l'accueil s'affiche aussi
//! vite qu'avant.

use std::sync::Mutex;

use super::Result;
use super::api::{API_V2, TidalClient};
use super::mix::RawMix;
use super::models::{HomeRow, MediaItem};
use super::raw::{RawAlbumFull, RawArtistFull, RawHome, RawHomePage, RawHomeRow, RawPlaylist};

/// Nombre maximal de pages lues (deux existent).
const MAX_PAGES: usize = 5;

/// Première page de l'accueil.
pub struct HomeStart {
    pub rows: Vec<HomeRow>,
    /// D'autres pages existent : `home_rest` les lit.
    pub more: bool,
}

/// Première page lue par `home_start`, avec le curseur de la suivante, gardée pour
/// `home_rest`.
#[derive(Default)]
pub(super) struct HomeCache(Mutex<Option<FirstPage>>);

struct FirstPage {
    user: Option<u64>,
    locale: &'static str,
    rows: Vec<HomeRow>,
    cursor: String,
}

impl TidalClient {
    /// Toutes les lignes de l'accueil, pages suivantes comprises. Seuls albums, artistes,
    /// playlists et mixes sont gardés (`TRACK` sans album et `DEEP_LINK` sont ignorés).
    /// Tests seulement : l'app passe par `home_start` puis `home_rest`.
    #[cfg(test)]
    pub async fn home(&self) -> Result<Vec<HomeRow>> {
        let start = self.home_start().await?;
        if !start.more {
            return Ok(start.rows);
        }
        self.home_rest().await
    }

    /// Première page seule ; `more` dit s'il faut appeler `home_rest`.
    pub async fn home_start(&self) -> Result<HomeStart> {
        let user = self.user_id().await.ok();
        let locale = crate::i18n::tidal_locale();
        let raw = self.home_page(locale, None).await?;
        let rows = home_rows(raw.items, user);
        let cursor = next_cursor(raw.page, &[]);
        let more = cursor.is_some();
        if let Ok(mut first) = self.home.0.lock() {
            *first = cursor.map(|cursor| FirstPage {
                user,
                locale,
                rows: rows.clone(),
                cursor,
            });
        }
        Ok(HomeStart { rows, more })
    }

    /// Toutes les lignes : la première page gardée par `home_start` (relue si elle manque
    /// ou ne correspond plus au compte ou à la langue), puis les suivantes. Une page
    /// suivante en échec est journalisée : l'accueil garde les précédentes.
    pub async fn home_rest(&self) -> Result<Vec<HomeRow>> {
        let user = self.user_id().await.ok();
        let locale = crate::i18n::tidal_locale();
        let first = self.home.0.lock().ok().and_then(|mut f| f.take());
        let (mut rows, cursor) = match first {
            Some(f) if f.user == user && f.locale == locale => (f.rows, Some(f.cursor)),
            _ => {
                let raw = self.home_page(locale, None).await?;
                (home_rows(raw.items, user), next_cursor(raw.page, &[]))
            }
        };
        let mut cursors: Vec<String> = cursor.into_iter().collect();
        while let Some(cursor) = cursors.last() {
            let raw = match self.home_page(locale, Some(cursor)).await {
                Ok(raw) => raw,
                Err(e) => {
                    tracing::warn!("page {} de l'accueil : {e}", cursors.len() + 1);
                    break;
                }
            };
            rows.extend(home_rows(raw.items, user));
            match next_cursor(raw.page, &cursors) {
                Some(next) => cursors.push(next),
                None => break,
            }
        }
        Ok(rows)
    }

    async fn home_page(&self, locale: &str, cursor: Option<&str>) -> Result<RawHome> {
        let mut query = vec![
            ("deviceType", "BROWSER".to_owned()),
            ("locale", locale.to_owned()),
            ("platform", "WEB".to_owned()),
        ];
        if let Some(cursor) = cursor {
            query.push(("cursor", cursor.to_owned()));
        }
        self.get_at(API_V2, "home/feed/static", &query).await
    }
}

/// Curseur de la page suivante, sauf s'il est vide, déjà suivi, ou si la limite de pages
/// est atteinte (`seen` = curseurs déjà suivis ; la première page n'en a pas).
fn next_cursor(page: Option<RawHomePage>, seen: &[String]) -> Option<String> {
    let cursor = page?.cursor.filter(|c| !c.trim().is_empty())?;
    (seen.len() + 1 < MAX_PAGES && !seen.contains(&cursor)).then_some(cursor)
}

/// Lignes gardées d'une page (une ligne sans élément lisible est retirée). Plusieurs
/// lignes peuvent partager une clé (« Parce que vous avez écouté ») : la disposition les
/// regroupe (`home_layout.rs`).
fn home_rows(raw: Vec<RawHomeRow>, user: Option<u64>) -> Vec<HomeRow> {
    raw.into_iter()
        .filter_map(|row| {
            let items: Vec<MediaItem> = row
                .items
                .into_iter()
                .filter_map(|it| home_item(&it.kind, it.data, user))
                .collect();
            let title = row.title.unwrap_or_default();
            (!items.is_empty()).then(|| HomeRow {
                key: row_key(row.module_id.as_deref(), &title),
                title,
                items,
            })
        })
        .collect()
}

/// Clé stable d'une ligne de l'accueil : le `moduleId` de l'API (le titre est traduit et
/// peut changer), sinon le titre normalisé, préfixé pour ne jamais croiser un `moduleId`.
fn row_key(module_id: Option<&str>, title: &str) -> String {
    match module_id.map(str::trim).filter(|m| !m.is_empty()) {
        Some(id) => id.to_owned(),
        None => format!("title:{}", title.trim().to_lowercase()),
    }
}

fn home_item(kind: &str, data: serde_json::Value, user: Option<u64>) -> Option<MediaItem> {
    match kind {
        "ALBUM" => serde_json::from_value::<RawAlbumFull>(data)
            .ok()
            .map(|a| MediaItem::Album(a.into())),
        "ARTIST" => serde_json::from_value::<RawArtistFull>(data)
            .ok()
            .map(|a| MediaItem::Artist(a.into())),
        "PLAYLIST" => serde_json::from_value::<RawPlaylist>(data)
            .ok()
            .map(|p| MediaItem::Playlist(p.into_summary(user))),
        "MIX" => serde_json::from_value::<RawMix>(data)
            .ok()
            .and_then(RawMix::into_summary)
            .map(MediaItem::Mix),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn page(cursor: Option<&str>) -> Option<RawHomePage> {
        Some(RawHomePage {
            cursor: cursor.map(str::to_owned),
        })
    }

    #[test]
    fn next_cursor_follows_until_end() {
        assert_eq!(next_cursor(page(Some("A")), &[]), Some("A".to_owned()));
        assert_eq!(next_cursor(page(None), &["A".into()]), None);
        assert_eq!(next_cursor(page(Some(" ")), &[]), None);
        assert_eq!(next_cursor(None, &[]), None);
        // Un curseur déjà suivi ne fait pas boucler.
        assert_eq!(next_cursor(page(Some("A")), &["A".into()]), None);
    }

    #[test]
    fn next_cursor_bounded() {
        let seen: Vec<String> = (1..MAX_PAGES).map(|i| i.to_string()).collect();
        let below = &seen[..seen.len() - 1];
        assert_eq!(next_cursor(page(Some("x")), below), Some("x".to_owned()));
        assert_eq!(next_cursor(page(Some("x")), &seen), None);
    }

    #[test]
    fn page_cursor_decoded() {
        let raw: RawHome =
            serde_json::from_value(json!({"items": [], "page": {"cursor": "U1VHR0VTVEVE"}}))
                .unwrap();
        assert_eq!(next_cursor(raw.page, &[]), Some("U1VHR0VTVEVE".to_owned()));
        let last: RawHome =
            serde_json::from_value(json!({"items": [], "page": {"cursor": null}})).unwrap();
        assert_eq!(next_cursor(last.page, &[]), None);
        let none: RawHome = serde_json::from_value(json!({"items": []})).unwrap();
        assert!(none.page.is_none());
    }

    #[test]
    fn rows_keep_shared_keys() {
        let album = |id: u64| json!({"type": "ALBUM", "data": {"id": id, "title": "a"}});
        let raw: RawHome = serde_json::from_value(json!({"items": [
            {"moduleId": "BECAUSE_YOU_LISTENED_TO_ALBUM", "title": "Parce que", "items": [album(1)]},
            {"moduleId": "BECAUSE_YOU_LISTENED_TO_ALBUM", "title": "Parce que", "items": [album(2)]},
            {"moduleId": "NEW_TRACK_SUGGESTIONS", "title": "Titres", "items": [{"type": "TRACK", "data": {}}]},
        ]}))
        .unwrap();
        let rows = home_rows(raw.items, None);
        assert_eq!(rows.len(), 2);
        assert!(
            rows.iter()
                .all(|r| r.key == "BECAUSE_YOU_LISTENED_TO_ALBUM")
        );
    }

    #[test]
    fn row_key_prefers_module_id() {
        assert_eq!(
            row_key(Some("YOUR_FAVORITE_ARTISTS"), "Vos artistes préférés"),
            "YOUR_FAVORITE_ARTISTS"
        );
        assert_eq!(row_key(Some("  "), " Pour Vous "), "title:pour vous");
        assert_eq!(row_key(None, "Pour vous"), "title:pour vous");
    }

    #[test]
    fn home_items_by_kind() {
        let album = json!({"id": 1, "title": "a", "cover": "x-y", "artists": [{"id": 2, "name": "b"}],
                           "mediaMetadata": {"tags": ["HIRES_LOSSLESS"]}});
        assert!(matches!(home_item("ALBUM", album, None), Some(MediaItem::Album(a)) if a.hires));
        let mix = json!({"id": "m", "type": "DISCOVERY_MIX", "titleTextInfo": {"text": "My Daily Discovery"}});
        assert!(
            matches!(home_item("MIX", mix, None), Some(MediaItem::Mix(m)) if m.title == "My Daily Discovery")
        );
        let untitled = json!({"id": "m"});
        assert_eq!(home_item("MIX", untitled, None), None);
        let broken = json!({"title": "sans id"});
        assert_eq!(home_item("ARTIST", broken, None), None);
    }

    /// L'étiquette `kind` de `MediaItem` ne doit entrer en conflit avec aucun champ.
    #[test]
    fn media_item_tag_is_unique() {
        let album = json!({"id": 1, "title": "a", "type": "SINGLE"});
        let item = home_item("ALBUM", album, None).unwrap();
        let text = serde_json::to_string(&item).unwrap();
        assert_eq!(text.matches("\"kind\"").count(), 1);
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["kind"], "album");
        assert_eq!(value["albumType"], "SINGLE");
    }
}
