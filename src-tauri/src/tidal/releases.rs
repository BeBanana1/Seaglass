//! Sorties récentes des artistes suivis (#415), ajoutées au fil d'actualité.
//!
//! Le fil de TIDAL (`feed.rs`) en oublie : comparé le 2026-11-13 aux albums des 35 artistes
//! suivis du compte de test (`cargo test live_feed_coverage -- --ignored --nocapture`), il
//! lui manquait notamment un album du 2026-11-05 et un single de septembre de l'artiste
//! principal. On lit donc aussi `artists/{id}/albums` (avec et sans
//! `filter=EPSANDSINGLES`), triés par date de sortie décroissante : les 10 premiers de
//! chaque liste suffisent. Deux requêtes par artiste (7 à 8 s en série pour 35 artistes),
//! d'où le parallélisme et la copie gardée 30 min.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::Result;
use super::api::TidalClient;
use super::date::date_ms;
use super::models::{AlbumSummary, FeedItem, MediaItem};
use super::raw::{RawAlbumFull, RawPage};

/// Sorties gardées : celles des 90 derniers jours (le fil remonte à environ 3 mois).
const WINDOW_DAYS: i64 = 90;
const DAY_MS: i64 = 86_400_000;
/// Éléments lus par liste (albums, puis EP et singles), les plus récents d'abord.
const PER_LIST: u32 = 10;
/// Artistes lus en même temps.
const PARALLEL: usize = 6;
/// Durée de vie de la copie : le fil est demandé à chaque visite et toutes les 6 h.
const TTL: Duration = Duration::from_secs(30 * 60);

/// Dernières sorties lues, par compte.
#[derive(Default)]
pub(super) struct ReleasesCache(Mutex<Option<Cached>>);

struct Cached {
    user: u64,
    at: Instant,
    albums: Vec<AlbumSummary>,
}

impl ReleasesCache {
    fn get(&self, user: u64) -> Option<Vec<AlbumSummary>> {
        let guard = self.0.lock().ok()?;
        let cached = guard.as_ref()?;
        (cached.user == user && cached.at.elapsed() < TTL).then(|| cached.albums.clone())
    }

    fn put(&self, user: u64, albums: Vec<AlbumSummary>) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = Some(Cached {
                user,
                at: Instant::now(),
                albums,
            });
        }
    }
}

impl TidalClient {
    /// Fil d'actualité complété des sorties récentes des artistes suivis, du plus récent au
    /// plus ancien. Si la lecture des artistes échoue, le fil seul est rendu.
    pub async fn feed_with_releases(self: &Arc<Self>, now_ms: i64) -> Result<Vec<FeedItem>> {
        let (feed, releases) = tokio::join!(self.feed(), self.followed_releases());
        let releases = releases.unwrap_or_else(|e| {
            tracing::warn!("sorties des artistes suivis illisibles : {e}");
            Vec::new()
        });
        Ok(merge(feed?, releases, now_ms))
    }

    async fn followed_releases(self: &Arc<Self>) -> Result<Vec<AlbumSummary>> {
        let user = self.user_id().await?;
        if let Some(albums) = self.releases.get(user) {
            return Ok(albums);
        }
        let artists = self.favorite_artists().await?;
        let ids: Vec<u64> = artists.items.iter().map(|a| a.id).collect();
        let mut albums = Vec::new();
        for chunk in ids.chunks(PARALLEL) {
            let mut set = tokio::task::JoinSet::new();
            for &id in chunk {
                let client = Arc::clone(self);
                set.spawn(async move { (id, client.artist_releases(id).await) });
            }
            while let Some(joined) = set.join_next().await {
                match joined {
                    Ok((_, Ok(list))) => albums.extend(list),
                    // Un artiste illisible n'empêche pas d'afficher les autres.
                    Ok((id, Err(e))) => tracing::warn!("sorties de l'artiste {id} : {e}"),
                    Err(e) => tracing::debug!("tâche interrompue : {e}"),
                }
            }
        }
        self.releases.put(user, albums.clone());
        Ok(albums)
    }

    /// Derniers albums, puis derniers EP et singles d'un artiste.
    async fn artist_releases(&self, id: u64) -> Result<Vec<AlbumSummary>> {
        let path = format!("artists/{id}/albums");
        let albums = [("limit", PER_LIST.to_string())];
        let singles = [
            ("limit", PER_LIST.to_string()),
            ("filter", "EPSANDSINGLES".to_owned()),
        ];
        let (albums, singles) = tokio::try_join!(
            self.get::<RawPage<RawAlbumFull>>(&path, &albums),
            self.get::<RawPage<RawAlbumFull>>(&path, &singles),
        )?;
        Ok(albums
            .items
            .into_iter()
            .chain(singles.items)
            .map(Into::into)
            .collect())
    }
}

/// Ajoute au fil les sorties des `WINDOW_DAYS` derniers jours qu'il n'a pas déjà, puis trie
/// du plus récent au plus ancien (sans date en dernier). Une sortie est déjà là si son id y
/// est, ou si un album du même artiste principal, du même type et au même titre y est
/// (autre édition : explicite et propre, rééditions du même jour).
fn merge(mut feed: Vec<FeedItem>, releases: Vec<AlbumSummary>, now_ms: i64) -> Vec<FeedItem> {
    let since = now_ms - WINDOW_DAYS * DAY_MS;
    let mut ids = HashSet::new();
    let mut titles = HashSet::new();
    for entry in &feed {
        if let MediaItem::Album(album) = &entry.item {
            ids.insert(album.id);
            titles.insert(title_key(album));
        }
    }
    for album in releases {
        let Some(at) = album.release_date.as_deref().and_then(date_ms) else {
            continue;
        };
        // Les sorties annoncées (date future) attendent leur jour.
        if at < since || at > now_ms {
            continue;
        }
        if !ids.insert(album.id) || !titles.insert(title_key(&album)) {
            continue;
        }
        feed.push(FeedItem {
            occurred_at: Some(at),
            seen: false,
            item: MediaItem::Album(album),
        });
    }
    // `Reverse(None)` passe après tous les `Reverse(Some(_))` ; tri stable.
    feed.sort_by_key(|entry| std::cmp::Reverse(entry.occurred_at));
    feed
}

fn title_key(album: &AlbumSummary) -> (u64, Option<String>, String) {
    (
        album.artists.first().map_or(0, |a| a.id),
        album.album_type.clone(),
        album.title.trim().to_lowercase(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::{ArtistRef, MixSummary};

    /// 2026-11-13 à midi UTC.
    fn now() -> i64 {
        date_ms("2026-11-13").unwrap() + 12 * 3_600_000
    }

    fn album(id: u64, artist: u64, title: &str, kind: &str, date: &str) -> AlbumSummary {
        AlbumSummary {
            id,
            title: title.to_owned(),
            artists: vec![ArtistRef {
                id: artist,
                name: format!("artiste {artist}"),
            }],
            cover_url: None,
            cover_large_url: None,
            color: None,
            release_date: Some(date.to_owned()),
            number_of_tracks: None,
            album_type: Some(kind.to_owned()),
            explicit: false,
            ai: false,
            hires: false,
        }
    }

    fn entry(at: Option<i64>, item: MediaItem) -> FeedItem {
        FeedItem {
            occurred_at: at,
            seen: true,
            item,
        }
    }

    fn ids(feed: &[FeedItem]) -> Vec<String> {
        feed.iter()
            .map(|f| match &f.item {
                MediaItem::Album(a) => a.id.to_string(),
                MediaItem::Mix(m) => m.id.clone(),
                _ => String::new(),
            })
            .collect()
    }

    #[test]
    fn adds_missing_releases_in_date_order() {
        let feed = vec![
            entry(
                Some(now() - DAY_MS),
                MediaItem::Album(album(1, 10, "Un", "ALBUM", "2026-11-12")),
            ),
            entry(
                Some(now() - 10 * DAY_MS),
                MediaItem::Mix(MixSummary {
                    id: "m".to_owned(),
                    title: "Mix".to_owned(),
                    subtitle: None,
                    image_url: None,
                    mix_type: None,
                }),
            ),
        ];
        let releases = vec![
            album(1, 10, "Un", "ALBUM", "2026-11-12"),
            album(2, 11, "Deux", "ALBUM", "2026-11-13"),
            album(3, 12, "Trois", "SINGLE", "2026-11-08"),
            // Hors fenêtre, à venir, sans date.
            album(4, 12, "Vieux", "ALBUM", "2026-08-14"),
            album(5, 12, "Annoncé", "ALBUM", "2026-11-14"),
            AlbumSummary {
                release_date: None,
                ..album(6, 12, "Sans date", "ALBUM", "")
            },
            // Même album lu chez deux artistes suivis.
            album(3, 12, "Trois", "SINGLE", "2026-11-08"),
        ];
        let merged = merge(feed, releases, now());
        assert_eq!(ids(&merged), ["2", "1", "3", "m"]);
        let added = &merged[0];
        assert_eq!(added.occurred_at, Some(now() - 12 * 3_600_000));
        assert!(!added.seen);
    }

    #[test]
    fn other_editions_are_not_repeated() {
        let feed = vec![entry(
            Some(now()),
            MediaItem::Album(album(1, 10, "Make It Easy", "SINGLE", "2026-11-13")),
        )];
        let releases = vec![
            // Version propre du même single : déjà là.
            album(2, 10, " make it easy ", "SINGLE", "2026-11-13"),
            // Même titre chez un autre artiste, ou album du même nom : à ajouter.
            album(3, 11, "Make It Easy", "SINGLE", "2026-11-13"),
            album(4, 10, "Make It Easy", "ALBUM", "2026-11-13"),
        ];
        assert_eq!(ids(&merge(feed, releases, now())), ["1", "3", "4"]);
    }

    #[test]
    fn undated_feed_items_stay_last() {
        let feed = vec![
            entry(None, MediaItem::Album(album(1, 10, "a", "ALBUM", "x"))),
            entry(
                Some(now()),
                MediaItem::Album(album(2, 10, "b", "ALBUM", "x")),
            ),
        ];
        assert_eq!(ids(&merge(feed, Vec::new(), now())), ["2", "1"]);
    }
}
