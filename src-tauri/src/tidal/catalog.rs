//! Catalogue : recherche, albums, artistes, playlists, favoris et accueil.
//!
//! Endpoints vérifiés avec le spike : `search` (multi-types), `albums/{id}`,
//! `albums/{id}/tracks`, `artists/{id}`, `artists/{id}/toptracks`,
//! `artists/{id}/albums[?filter=EPSANDSINGLES]`, `playlists/{uuid}`,
//! `playlists/{uuid}/tracks`, `users/{id}/favorites/{tracks,albums,artists}`,
//! `users/{id}/playlistsAndFavoritePlaylists`, v2 `home/feed/static`.
//! Mixes et radios : `mix.rs` ; pagination (`get_all`) : `paging.rs`.

use serde::de::DeserializeOwned;

use super::api::{API_V2, TidalClient};
use super::mix::RawMix;
use super::models::{
    AlbumPage, AlbumSummary, ArtistPage, ArtistSummary, HomeRow, LibraryList, MediaItem,
    PlaylistPage, PlaylistSummary, SearchResults, Track, TrackEntry,
};
use super::raw::{
    self, RawAlbumFull, RawArtistFull, RawFavorite, RawHome, RawPage, RawPlaylist,
    RawPlaylistEntry, RawSearch, RawTrack, image_url,
};
use super::{Result, TidalError};

/// Taille de page des listes paginées.
const PAGE_SIZE: u32 = 100;
/// `playlistsAndFavoritePlaylists` refuse plus de 50 éléments par page (vérifié).
const PLAYLISTS_PAGE_SIZE: u32 = 50;
/// Plafond des titres chargés pour un album ou une playlist.
const MAX_TRACKS: usize = 10_000;
/// Plafond de chaque catégorie de la bibliothèque (#91 : 2 000 avant la pagination
/// parallèle). Lire toute la liste doit rester sous `MAX_CONTEXT_LEN` (`commands/player.rs`).
pub const LIBRARY_MAX: usize = 10_000;
const TOP_TRACKS: u32 = 10;
const ARTIST_ALBUMS: u32 = 50;

impl TidalClient {
    pub async fn search(&self, query: &str, limit: u32) -> Result<SearchResults> {
        let raw: RawSearch = self
            .get(
                "search",
                &[
                    ("query", query.to_owned()),
                    ("types", "TRACKS,ALBUMS,ARTISTS,PLAYLISTS".to_owned()),
                    ("limit", limit.to_string()),
                ],
            )
            .await?;
        let user = self.user_id().await.ok();
        let tracks = self.remember(raw::tracks(raw.tracks.items));
        Ok(SearchResults {
            tracks,
            albums: raw.albums.items.into_iter().map(Into::into).collect(),
            artists: raw.artists.items.into_iter().map(Into::into).collect(),
            playlists: raw
                .playlists
                .items
                .into_iter()
                .map(|p| p.into_summary(user))
                .collect(),
        })
    }

    pub async fn album(&self, id: u64) -> Result<AlbumPage> {
        let (path, tracks_path) = (format!("albums/{id}"), format!("albums/{id}/tracks"));
        let (raw, tracks) = tokio::try_join!(
            self.get::<RawAlbumFull>(&path, &[]),
            self.get_all::<RawTrack>(&tracks_path, &[], PAGE_SIZE, MAX_TRACKS),
        )?;
        let cover_large_url = raw.cover.as_deref().map(|c| image_url(c, 640));
        let (duration, copyright) = (raw.duration, raw.copyright.clone());
        Ok(AlbumPage {
            album: raw.into(),
            cover_large_url,
            duration,
            copyright,
            tracks: self.remember(raw::tracks(tracks)),
        })
    }

    pub async fn artist(&self, id: u64) -> Result<ArtistPage> {
        let top = [("limit", TOP_TRACKS.to_string())];
        let albums = [("limit", ARTIST_ALBUMS.to_string())];
        let singles = [
            ("limit", ARTIST_ALBUMS.to_string()),
            ("filter", "EPSANDSINGLES".to_owned()),
        ];
        let path = format!("artists/{id}");
        let (top_path, albums_path) = (format!("{path}/toptracks"), format!("{path}/albums"));
        let (artist, top_tracks, albums, singles) = tokio::try_join!(
            self.get::<RawArtistFull>(&path, &[]),
            self.get::<RawPage<RawTrack>>(&top_path, &top),
            self.get::<RawPage<RawAlbumFull>>(&albums_path, &albums),
            self.get::<RawPage<RawAlbumFull>>(&albums_path, &singles),
        )?;
        Ok(ArtistPage {
            artist: artist.into(),
            top_tracks: self.remember(raw::tracks(top_tracks.items)),
            albums: albums.items.into_iter().map(Into::into).collect(),
            singles: singles.items.into_iter().map(Into::into).collect(),
        })
    }

    pub async fn playlist(&self, uuid: &str) -> Result<PlaylistPage> {
        let (path, tracks_path) = (
            format!("playlists/{uuid}"),
            format!("playlists/{uuid}/tracks"),
        );
        let (raw, tracks) = tokio::try_join!(
            self.get::<RawPlaylist>(&path, &[]),
            self.get_all::<RawTrack>(&tracks_path, &[], PAGE_SIZE, MAX_TRACKS),
        )?;
        let user = self.user_id().await.ok();
        Ok(PlaylistPage {
            playlist: raw.into_summary(user),
            tracks: self.remember_entries(raw::playlist_entries(tracks)),
        })
    }

    /// Titres favoris, du plus récent au plus ancien, avec la date du like.
    pub async fn favorite_tracks(&self) -> Result<LibraryList<TrackEntry>> {
        let raw: LibraryList<RawFavorite<RawTrack>> = self.favorites("tracks").await?;
        let list = map_list(raw, |items| {
            self.remember_entries(raw::favorite_entries(items))
        });
        // Liste complète : elle fait foi pour les cœurs (likes faits sur un autre appareil).
        if !list.truncated {
            self.likes.replace(list.items.iter().map(|e| e.track.id));
        }
        Ok(list)
    }

    pub async fn favorite_albums(&self) -> Result<LibraryList<AlbumSummary>> {
        let raw: LibraryList<RawFavorite<RawAlbumFull>> = self.favorites("albums").await?;
        Ok(map_list(raw, |items| {
            items.into_iter().map(|f| f.item.into()).collect()
        }))
    }

    pub async fn favorite_artists(&self) -> Result<LibraryList<ArtistSummary>> {
        let raw: LibraryList<RawFavorite<RawArtistFull>> = self.favorites("artists").await?;
        Ok(map_list(raw, |items| {
            items.into_iter().map(|f| f.item.into()).collect()
        }))
    }

    /// Playlists créées par l'utilisateur et playlists suivies.
    pub async fn library_playlists(&self) -> Result<LibraryList<PlaylistSummary>> {
        let user = self.user_id().await?;
        let raw: LibraryList<RawPlaylistEntry> = self
            .get_list(
                &format!("users/{user}/playlistsAndFavoritePlaylists"),
                &[],
                PLAYLISTS_PAGE_SIZE,
                LIBRARY_MAX,
            )
            .await?;
        Ok(map_list(raw, |items| {
            items
                .into_iter()
                .map(|e| e.playlist.into_summary(Some(user)))
                .collect()
        }))
    }

    /// Lignes de l'accueil personnalisé. Seuls albums, artistes, playlists et mixes sont
    /// gardés (`TRACK` sans album et `DEEP_LINK` sont ignorés).
    pub async fn home(&self) -> Result<Vec<HomeRow>> {
        let raw: RawHome = self
            .get_at(
                API_V2,
                "home/feed/static",
                &[
                    ("deviceType", "BROWSER".to_owned()),
                    ("locale", crate::i18n::tidal_locale().to_owned()),
                    ("platform", "WEB".to_owned()),
                ],
            )
            .await?;
        let user = self.user_id().await.ok();
        Ok(raw
            .items
            .into_iter()
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
            .collect())
    }

    async fn favorites<T: DeserializeOwned>(&self, kind: &str) -> Result<LibraryList<T>> {
        let user = self.user_id().await?;
        self.get_list(
            &format!("users/{user}/favorites/{kind}"),
            &[
                ("order", "DATE".to_owned()),
                ("orderDirection", "DESC".to_owned()),
            ],
            PAGE_SIZE,
            LIBRARY_MAX,
        )
        .await
    }

    pub(super) async fn user_id(&self) -> Result<u64> {
        self.session()
            .await
            .map(|s| s.user_id)
            .ok_or(TidalError::NotLoggedIn)
    }

    /// Ajoute les titres au cache (affichage de la file) et les renvoie.
    pub(super) fn remember(&self, tracks: Vec<Track>) -> Vec<Track> {
        self.cache.insert_all(&tracks);
        tracks
    }

    /// Comme `remember`, pour des titres de liste (seul le `Track` va dans le cache).
    fn remember_entries(&self, entries: Vec<TrackEntry>) -> Vec<TrackEntry> {
        self.cache.insert_all(entries.iter().map(|e| &e.track));
        entries
    }
}

/// Convertit les éléments d'une liste en gardant ce qu'elle dit de sa troncature.
fn map_list<A, B>(list: LibraryList<A>, convert: impl FnOnce(Vec<A>) -> Vec<B>) -> LibraryList<B> {
    LibraryList {
        items: convert(list.items),
        truncated: list.truncated,
        total: list.total,
    }
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
