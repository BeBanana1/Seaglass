//! Pages de playlist, gardées en mémoire avec leur ETag (#152).
//!
//! Lire une playlist, c'est son en-tête (`playlists/{uuid}`) plus toutes les pages de
//! `playlists/{uuid}/tracks` : 0,6–0,85 s pour 484 titres. L'ETag de l'en-tête change à
//! chaque modification (vérifié par `cargo test live_playlist_etag -- --ignored`) : une
//! page déjà lue est revalidée par l'en-tête seul, et ses titres ne sont relus que si
//! l'ETag a changé. L'UI affiche d'abord la copie gardée (`cached_playlist`), puis demande
//! si la playlist a changé depuis (`playlist_since`). Les pages survivent aussi au
//! redémarrage, dans la base (`commands::snapshots`, #181), et reviennent ici par
//! `restore_playlist`.

use std::sync::Mutex;

use super::api::{API_V1, Call, TidalClient};
use super::catalog::{MAX_TRACKS, PAGE_SIZE};
use super::models::PlaylistPage;
use super::raw::{self, RawPlaylist, RawTrack};
use super::{Result, TidalError};

/// Playlists gardées au plus.
const MAX_PLAYLISTS: usize = 20;
/// Titres gardés au plus, toutes playlists confondues (la plus récente est toujours gardée).
const MAX_CACHED_TRACKS: usize = 20_000;

impl TidalClient {
    /// Page complète de la playlist, revalidée par son ETag si elle est déjà gardée.
    // L'app passe par `playlist_since` ; sert aux tests réels.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn playlist(&self, uuid: &str) -> Result<PlaylistPage> {
        self.playlist_since(uuid, None).await?.ok_or_else(|| {
            TidalError::Decode(format!("playlists/{uuid} : page absente sans révision"))
        })
    }

    /// Copie gardée, sans réseau.
    pub fn cached_playlist(&self, uuid: &str) -> Option<PlaylistPage> {
        let page = self.playlists.get(uuid)?;
        // Ses titres repassent dans le cache des métadonnées : la lecture peut partir
        // avant la revalidation.
        self.cache.insert_all(page.tracks.iter().map(|e| &e.track));
        Some(page)
    }

    /// Copie relue sur le disque au lancement suivant (#181) : gardée comme une page lue,
    /// pour que la revalidation par ETag reprenne ses titres s'ils n'ont pas changé.
    pub fn restore_playlist(&self, uuid: &str, page: PlaylistPage) {
        if page.revision.is_none() {
            return;
        }
        self.cache.insert_all(page.tracks.iter().map(|e| &e.track));
        self.playlists.put(uuid, page);
    }

    /// `None` si la playlist n'a pas changé depuis `known` (sa `revision` déjà affichée),
    /// sinon la page à jour.
    pub async fn playlist_since(
        &self,
        uuid: &str,
        known: Option<&str>,
    ) -> Result<Option<PlaylistPage>> {
        let cached = self.playlists.get(uuid);
        if known.is_none() && cached.is_none() {
            // Rien à revalider : en-tête et titres en parallèle.
            let tracks_path = format!("playlists/{uuid}/tracks");
            let ((head, etag), tracks) = tokio::try_join!(
                self.playlist_head(uuid),
                self.get_all::<RawTrack>(&tracks_path, &[], PAGE_SIZE, MAX_TRACKS),
            )?;
            return Ok(Some(self.keep(uuid, head, etag, tracks).await));
        }
        let (head, etag) = self.playlist_head(uuid).await?;
        if etag.is_some() && etag.as_deref() == known {
            return Ok(None);
        }
        let user = self.user_id().await.ok();
        if let Some(mut page) = cached.filter(|c| c.revision.is_some() && c.revision == etag) {
            // Mêmes titres ; l'en-tête relu est repris (nombre de titres, image).
            page.playlist = head.into_summary(user);
            self.cache.insert_all(page.tracks.iter().map(|e| &e.track));
            return Ok(Some(page));
        }
        let tracks_path = format!("playlists/{uuid}/tracks");
        let tracks = self
            .get_all::<RawTrack>(&tracks_path, &[], PAGE_SIZE, MAX_TRACKS)
            .await?;
        Ok(Some(self.keep(uuid, head, etag, tracks).await))
    }

    /// En-tête de la playlist et son ETag.
    async fn playlist_head(&self, uuid: &str) -> Result<(RawPlaylist, Option<String>)> {
        let path = format!("playlists/{uuid}");
        let resp = self.call(&Call::get(API_V1, &path, &[])).await?;
        let etag = resp
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let head = resp
            .json()
            .await
            .map_err(|e| TidalError::Decode(format!("{path} : {e}")))?;
        Ok((head, etag))
    }

    async fn keep(
        &self,
        uuid: &str,
        head: RawPlaylist,
        etag: Option<String>,
        tracks: Vec<RawTrack>,
    ) -> PlaylistPage {
        let user = self.user_id().await.ok();
        let page = PlaylistPage {
            playlist: head.into_summary(user),
            tracks: self.remember_entries(raw::playlist_entries(tracks)),
            revision: etag,
        };
        // Sans ETag, rien ne permettrait de savoir si la copie est encore juste.
        if page.revision.is_some() {
            self.playlists.put(uuid, page.clone());
        } else {
            self.playlists.remove(uuid);
        }
        page
    }
}

/// Dernières pages lues, la plus récente en tête.
#[derive(Default)]
pub(super) struct PlaylistCache {
    entries: Mutex<Vec<(String, PlaylistPage)>>,
}

impl PlaylistCache {
    fn get(&self, uuid: &str) -> Option<PlaylistPage> {
        let mut entries = self.entries.lock().ok()?;
        let i = entries.iter().position(|(u, _)| u == uuid)?;
        let entry = entries.remove(i);
        let page = entry.1.clone();
        entries.insert(0, entry);
        Some(page)
    }

    fn put(&self, uuid: &str, page: PlaylistPage) {
        let Ok(mut entries) = self.entries.lock() else {
            return;
        };
        entries.retain(|(u, _)| u != uuid);
        entries.insert(0, (uuid.to_owned(), page));
        trim(&mut entries);
    }

    /// Playlist modifiée par l'app : la copie gardée ne vaut plus rien.
    pub(super) fn remove(&self, uuid: &str) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.retain(|(u, _)| u != uuid);
        }
    }

    /// Déconnexion : `mine` dépend du compte.
    pub(super) fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }
}

/// Garde au plus `MAX_PLAYLISTS` pages et `MAX_CACHED_TRACKS` titres, en partant des plus
/// récentes ; la première est gardée même si elle dépasse seule la limite des titres.
fn trim(entries: &mut Vec<(String, PlaylistPage)>) {
    let mut total = 0;
    let keep = entries
        .iter()
        .take(MAX_PLAYLISTS)
        .position(|(_, p)| {
            total += p.tracks.len();
            total > MAX_CACHED_TRACKS
        })
        .map_or(MAX_PLAYLISTS, |i| i.max(1));
    entries.truncate(keep);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::{AlbumRef, PlaylistSummary, Track, TrackEntry, TrackId};

    fn page(uuid: &str, tracks: usize) -> PlaylistPage {
        let track = Track {
            id: TrackId(1),
            title: "t".into(),
            version: None,
            duration: 100,
            artists: Vec::new(),
            album: AlbumRef {
                id: 1,
                title: "a".into(),
                cover_url: None,
                cover_small_url: None,
                cover_large_url: None,
                color: None,
            },
            explicit: false,
            hires: false,
            isrc: None,
        };
        PlaylistPage {
            playlist: PlaylistSummary {
                uuid: uuid.to_owned(),
                title: uuid.to_owned(),
                description: None,
                image_url: None,
                number_of_tracks: None,
                duration: None,
                editorial: false,
                mine: false,
            },
            tracks: vec![
                TrackEntry {
                    track,
                    added_at: None,
                };
                tracks
            ],
            revision: Some(format!("etag-{uuid}")),
        }
    }

    fn uuids(cache: &PlaylistCache) -> Vec<String> {
        let entries = cache.entries.lock().unwrap();
        entries.iter().map(|(u, _)| u.clone()).collect()
    }

    #[test]
    fn get_moves_to_front() {
        let cache = PlaylistCache::default();
        cache.put("a", page("a", 1));
        cache.put("b", page("b", 1));
        assert_eq!(uuids(&cache), ["b", "a"]);
        assert_eq!(cache.get("a").unwrap().revision.as_deref(), Some("etag-a"));
        assert_eq!(uuids(&cache), ["a", "b"]);
        assert!(cache.get("c").is_none());
    }

    #[test]
    fn put_replaces_and_remove_forgets() {
        let cache = PlaylistCache::default();
        cache.put("a", page("a", 1));
        cache.put("b", page("b", 1));
        cache.put("a", page("a", 3));
        assert_eq!(uuids(&cache), ["a", "b"]);
        assert_eq!(cache.get("a").unwrap().tracks.len(), 3);
        cache.remove("a");
        assert_eq!(uuids(&cache), ["b"]);
        cache.clear();
        assert!(uuids(&cache).is_empty());
    }

    #[test]
    fn bounded_by_count() {
        let cache = PlaylistCache::default();
        for i in 0..MAX_PLAYLISTS + 5 {
            cache.put(&i.to_string(), page(&i.to_string(), 1));
        }
        let kept = uuids(&cache);
        assert_eq!(kept.len(), MAX_PLAYLISTS);
        assert_eq!(kept[0], (MAX_PLAYLISTS + 4).to_string());
    }

    #[test]
    fn bounded_by_tracks_keeping_the_latest() {
        let cache = PlaylistCache::default();
        cache.put("a", page("a", MAX_CACHED_TRACKS / 2));
        cache.put("b", page("b", MAX_CACHED_TRACKS / 2));
        assert_eq!(uuids(&cache), ["b", "a"]);
        cache.put("c", page("c", 1));
        assert_eq!(uuids(&cache), ["c", "b"]);
        // Une playlist plus grande que la limite reste gardée, seule.
        cache.put("huge", page("huge", MAX_CACHED_TRACKS + 1));
        assert_eq!(uuids(&cache), ["huge"]);
    }
}
