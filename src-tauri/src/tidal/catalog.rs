//! Catalogue : recherche, albums, artistes, playlists et favoris (accueil : `home.rs`).
//!
//! Endpoints vérifiés avec le spike : `search` (multi-types), `albums/{id}`,
//! `albums/{id}/tracks`, `artists/{id}`, `artists/{id}/toptracks`,
//! `artists/{id}/albums[?filter=EPSANDSINGLES]`, `playlists/{uuid}`,
//! `playlists/{uuid}/tracks`, `users/{id}/favorites/{tracks,albums,artists}`,
//! `users/{id}/playlistsAndFavoritePlaylists`.
//! Mixes et radios : `mix.rs` ; pages de playlist : `playlist.rs` ; pagination (`get_all`) :
//! `paging.rs`.

use serde::de::DeserializeOwned;

use super::api::TidalClient;
use super::likes::LikedKind;
use super::models::{
    AlbumPage, AlbumSummary, ArtistPage, ArtistSummary, LibraryList, PlaylistSummary,
    SearchResults, Track, TrackEntry,
};
use super::raw::{
    self, RawAlbumFull, RawArtistFull, RawFavorite, RawPage, RawPlaylistEntry, RawSearch, RawTrack,
    image_url,
};
use super::{Result, TidalError};

/// Taille de page des listes paginées.
pub(super) const PAGE_SIZE: u32 = 100;
/// `playlistsAndFavoritePlaylists` refuse plus de 50 éléments par page (vérifié).
const PLAYLISTS_PAGE_SIZE: u32 = 50;
/// Plafond des titres chargés pour un album ou une playlist.
pub(super) const MAX_TRACKS: usize = 10_000;
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

    /// Titres favoris, du plus récent au plus ancien, avec la date du like.
    pub async fn favorite_tracks(&self) -> Result<LibraryList<TrackEntry>> {
        let raw: LibraryList<RawFavorite<RawTrack>> = self.favorites("tracks").await?;
        let list = map_list(raw, |items| {
            self.remember_entries(raw::favorite_entries(items))
        });
        // Liste complète : elle fait foi pour les cœurs (likes faits sur un autre appareil).
        if !list.truncated {
            let ids = list.items.iter().map(|e| e.track.id).collect();
            self.likes.replace(LikedKind::Tracks(ids));
        }
        Ok(list)
    }

    pub async fn favorite_albums(&self) -> Result<LibraryList<AlbumSummary>> {
        let raw: LibraryList<RawFavorite<RawAlbumFull>> = self.favorites("albums").await?;
        let list: LibraryList<AlbumSummary> = map_list(raw, |items| {
            items.into_iter().map(|f| f.item.into()).collect()
        });
        if !list.truncated {
            let ids = list.items.iter().map(|a| a.id).collect();
            self.likes.replace(LikedKind::Albums(ids));
        }
        Ok(list)
    }

    pub async fn favorite_artists(&self) -> Result<LibraryList<ArtistSummary>> {
        let raw: LibraryList<RawFavorite<RawArtistFull>> = self.favorites("artists").await?;
        let list: LibraryList<ArtistSummary> = map_list(raw, |items| {
            items.into_iter().map(|f| f.item.into()).collect()
        });
        if !list.truncated {
            let ids = list.items.iter().map(|a| a.id).collect();
            self.likes.replace(LikedKind::Artists(ids));
        }
        Ok(list)
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
        let list: LibraryList<PlaylistSummary> = map_list(raw, |items| {
            items
                .into_iter()
                .map(|e| e.playlist.into_summary(Some(user)))
                .collect()
        });
        // Seules les playlists des autres se suivent ; les vôtres n'ont pas de bouton.
        if !list.truncated {
            let followed = list.items.iter().filter(|p| !p.mine);
            self.likes.replace(LikedKind::Playlists(
                followed.map(|p| p.uuid.clone()).collect(),
            ));
        }
        Ok(list)
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
    pub(super) fn remember_entries(&self, entries: Vec<TrackEntry>) -> Vec<TrackEntry> {
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
