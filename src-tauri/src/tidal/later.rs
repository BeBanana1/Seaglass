//! « Plus tard » (Save for later, #196) : liste de TIDAL où l'on met de côté titres, albums,
//! artistes et playlists. Elle n'existe que dans l'API officielle JSON:API
//! (`openapi.tidal.com/v2`), qui accepte le token du client PKCE. Vérifié le 2026-11-05
//! (`cargo test live_later -- --ignored --nocapture`) :
//!
//! - GET `userCollectionSaveForLaters/me/relationships/items` → `{data: [{id, type, meta:
//!   {addedAt}}], links: {next, meta: {nextCursor}}}`, plus récent en tête, 20 par page,
//!   page suivante par `page[cursor]` ; `type` vaut `tracks`, `albums`, `artists`,
//!   `playlists` ou `videos` (ignorées : l'app ne lit pas de vidéos).
//! - POST (ajout) et DELETE (retrait) sur le même chemin, corps `{data: [{id, type}]}` en
//!   `application/vnd.api+json`, 20 éléments au plus. Un doublon revient dans
//!   `meta.skipped` (`ALREADY_PRESENT`) sans erreur ; le retrait est idempotent.
//!
//! Seuls les identifiants viennent de cette API : titres, albums, artistes et playlists sont
//! relus par l'API v1, comme le reste de l'app (le cache des titres sert d'abord).

use std::collections::HashMap;

use reqwest::Method;
use serde::Deserialize;

use super::api::{Call, OPEN_API};
use super::date::timestamp_ms;
use super::models::{
    AlbumSummary, ArtistSummary, LaterItem, LibraryList, LikeTarget, LikedIds, PlaylistSummary,
    TrackEntry, TrackId,
};
use super::paging::try_join_bounded;
use super::raw::{RawAlbumFull, RawArtistFull, RawPlaylist};
use super::{Result, TidalClient, TidalError};

const ITEMS_PATH: &str = "userCollectionSaveForLaters/me/relationships/items";
/// Pages lues au plus (20 éléments chacune) : au-delà, la liste est dite coupée.
const MAX_PAGES: usize = 50;
/// Requêtes de résolution en vol à la fois.
const PARALLEL: usize = 6;
/// Albums par requête `albums?ids=`.
const ALBUM_CHUNK: usize = 50;

/// Un élément de la liste, tel que l'API le décrit.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum LaterRef {
    Track(TrackId),
    Album(u64),
    Artist(u64),
    Playlist(String),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct LaterEntry {
    pub target: LaterRef,
    pub added_at: Option<i64>,
}

impl TidalClient {
    /// Identifiants de la liste, chargés au premier appel puis tenus à jour.
    pub async fn later_ids(&self) -> Result<LikedIds> {
        if let Some(ids) = self.later.get() {
            return Ok(ids);
        }
        let (entries, _) = self.later_entries().await?;
        self.later.load(ids_of(&entries));
        Ok(self.later.get().unwrap_or_default())
    }

    /// Toute la liste, du plus récent au plus ancien ; les éléments devenus indisponibles
    /// sont omis.
    pub async fn save_for_later(&self) -> Result<LibraryList<LaterItem>> {
        let (entries, truncated) = self.later_entries().await?;
        self.later.load(ids_of(&entries));
        let items = self.resolve(&entries).await?;
        Ok(LibraryList {
            total: Some(u32::try_from(entries.len()).unwrap_or(u32::MAX)),
            items,
            truncated,
        })
    }

    /// Ajoute à la liste ou retire. Les mixes n'y vont pas (refusés par l'API).
    pub async fn set_saved_for_later(&self, target: &LikeTarget, saved: bool) -> Result<()> {
        let (kind, id) = match target {
            LikeTarget::Track { id } => ("tracks", id.to_string()),
            LikeTarget::Album { id } => ("albums", id.to_string()),
            LikeTarget::Artist { id } => ("artists", id.to_string()),
            LikeTarget::Playlist { uuid } => ("playlists", uuid.clone()),
            LikeTarget::Mix { .. } => {
                return Err(TidalError::Decode("mix : pas dans « Plus tard »".into()));
            }
        };
        let body = serde_json::json!({ "data": [{ "id": id, "type": kind }] });
        let mut call = Call::get(OPEN_API, ITEMS_PATH, &[]);
        call.method = if saved { Method::POST } else { Method::DELETE };
        call.json = Some(&body);
        self.call(&call).await?;
        self.later.set(target, saved);
        Ok(())
    }

    /// Toutes les pages d'identifiants ; `true` si le plafond de pages a coupé la liste.
    async fn later_entries(&self) -> Result<(Vec<LaterEntry>, bool)> {
        let mut entries = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let query: Vec<(&str, String)> = cursor
                .take()
                .map(|c| vec![("page[cursor]", c)])
                .unwrap_or_default();
            let page: RawItems = self
                .call(&Call::get(OPEN_API, ITEMS_PATH, &query))
                .await?
                .json()
                .await?;
            entries.extend(page.data.into_iter().filter_map(parse_entry));
            match page.links.and_then(RawLinks::next_cursor) {
                Some(next) => cursor = Some(next),
                None => return Ok((entries, false)),
            }
        }
        Ok((entries, true))
    }

    /// Relit chaque élément par l'API v1, dans l'ordre de la liste.
    async fn resolve(&self, entries: &[LaterEntry]) -> Result<Vec<LaterItem>> {
        let mut tracks = Vec::new();
        let mut albums = Vec::new();
        let mut artists = Vec::new();
        let mut playlists = Vec::new();
        for entry in entries {
            match &entry.target {
                LaterRef::Track(id) => tracks.push(*id),
                LaterRef::Album(id) => albums.push(*id),
                LaterRef::Artist(id) => artists.push(*id),
                LaterRef::Playlist(uuid) => playlists.push(uuid.as_str()),
            }
        }
        let (tracks, albums, artists, playlists) = tokio::try_join!(
            self.later_tracks(&tracks),
            self.later_albums(&albums),
            self.later_artists(&artists),
            self.later_playlists(&playlists),
        )?;
        Ok(assemble(entries, tracks, albums, artists, playlists))
    }

    async fn later_tracks(&self, ids: &[TrackId]) -> Result<HashMap<TrackId, TrackEntry>> {
        let mut futures = Vec::new();
        for &id in ids {
            futures.push(async move { gone_as_none(self.track(id).await) });
        }
        let found = try_join_bounded(futures, PARALLEL).await?;
        Ok(found
            .into_iter()
            .flatten()
            .map(|track| {
                let entry = TrackEntry {
                    track,
                    added_at: None,
                };
                (entry.track.id, entry)
            })
            .collect())
    }

    async fn later_albums(&self, ids: &[u64]) -> Result<HashMap<u64, AlbumSummary>> {
        let mut futures = Vec::new();
        for chunk in ids.chunks(ALBUM_CHUNK) {
            let query = [(
                "ids",
                chunk
                    .iter()
                    .map(u64::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            )];
            futures.push(async move { self.get::<Vec<RawAlbumFull>>("albums", &query).await });
        }
        let found = try_join_bounded(futures, PARALLEL).await?;
        Ok(found
            .into_iter()
            .flatten()
            .map(|raw| {
                let album: AlbumSummary = raw.into();
                (album.id, album)
            })
            .collect())
    }

    async fn later_artists(&self, ids: &[u64]) -> Result<HashMap<u64, ArtistSummary>> {
        let mut futures = Vec::new();
        for &id in ids {
            let path = format!("artists/{id}");
            futures.push(async move { gone_as_none(self.get::<RawArtistFull>(&path, &[]).await) });
        }
        let found = try_join_bounded(futures, PARALLEL).await?;
        Ok(found
            .into_iter()
            .flatten()
            .map(|raw| {
                let artist: ArtistSummary = raw.into();
                (artist.id, artist)
            })
            .collect())
    }

    async fn later_playlists(&self, uuids: &[&str]) -> Result<HashMap<String, PlaylistSummary>> {
        let user = self.user_id().await.ok();
        let mut futures = Vec::new();
        for uuid in uuids {
            let path = format!("playlists/{uuid}");
            futures.push(async move { gone_as_none(self.get::<RawPlaylist>(&path, &[]).await) });
        }
        let found = try_join_bounded(futures, PARALLEL).await?;
        Ok(found
            .into_iter()
            .flatten()
            .map(|raw| {
                let playlist = raw.into_summary(user);
                (playlist.uuid.clone(), playlist)
            })
            .collect())
    }
}

/// Un élément introuvable ou illisible (retiré du catalogue) est omis ; une erreur réseau
/// ou de session fait échouer toute la liste.
fn gone_as_none<T>(result: Result<T>) -> Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(TidalError::Api { status, .. }) if (400..500).contains(&status) && status != 401 => {
            Ok(None)
        }
        Err(TidalError::Decode(e)) => {
            tracing::debug!("élément de « Plus tard » illisible : {e}");
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

fn ids_of(entries: &[LaterEntry]) -> LikedIds {
    let mut ids = LikedIds::default();
    for entry in entries {
        match &entry.target {
            LaterRef::Track(id) => ids.tracks.push(*id),
            LaterRef::Album(id) => ids.albums.push(*id),
            LaterRef::Artist(id) => ids.artists.push(*id),
            LaterRef::Playlist(uuid) => ids.playlists.push(uuid.clone()),
        }
    }
    ids
}

/// Remet les éléments relus dans l'ordre de la liste ; un titre garde sa date d'ajout.
fn assemble(
    entries: &[LaterEntry],
    mut tracks: HashMap<TrackId, TrackEntry>,
    mut albums: HashMap<u64, AlbumSummary>,
    mut artists: HashMap<u64, ArtistSummary>,
    mut playlists: HashMap<String, PlaylistSummary>,
) -> Vec<LaterItem> {
    let mut items = Vec::with_capacity(entries.len());
    for entry in entries {
        let item = match &entry.target {
            // `remove` : un doublon éventuel n'apparaît qu'une fois.
            LaterRef::Track(id) => tracks.remove(id).map(|mut t| {
                t.added_at = entry.added_at;
                LaterItem::Track(t)
            }),
            LaterRef::Album(id) => albums.remove(id).map(LaterItem::Album),
            LaterRef::Artist(id) => artists.remove(id).map(LaterItem::Artist),
            LaterRef::Playlist(uuid) => playlists.remove(uuid).map(LaterItem::Playlist),
        };
        items.extend(item);
    }
    items
}

fn parse_entry(raw: RawRef) -> Option<LaterEntry> {
    let target = match raw.kind.as_str() {
        "tracks" => LaterRef::Track(TrackId(raw.id.parse().ok()?)),
        "albums" => LaterRef::Album(raw.id.parse().ok()?),
        "artists" => LaterRef::Artist(raw.id.parse().ok()?),
        // L'uuid finit dans un chemin de requête : il est vérifié.
        "playlists" if is_uuid(&raw.id) => LaterRef::Playlist(raw.id),
        _ => return None,
    };
    Some(LaterEntry {
        target,
        added_at: raw
            .meta
            .and_then(|m| m.added_at)
            .as_deref()
            .and_then(timestamp_ms),
    })
}

/// `8-4-4-4-12` caractères hexadécimaux.
fn is_uuid(s: &str) -> bool {
    let groups: Vec<&str> = s.split('-').collect();
    groups.len() == 5
        && groups
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(g, n)| g.len() == n && g.chars().all(|c| c.is_ascii_hexdigit()))
}

#[derive(Deserialize)]
struct RawItems {
    #[serde(default)]
    data: Vec<RawRef>,
    links: Option<RawLinks>,
}

#[derive(Deserialize)]
struct RawRef {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    meta: Option<RawRefMeta>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRefMeta {
    added_at: Option<String>,
}

#[derive(Deserialize)]
struct RawLinks {
    next: Option<String>,
    meta: Option<RawLinksMeta>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLinksMeta {
    next_cursor: Option<String>,
}

impl RawLinks {
    /// Curseur de la page suivante, seulement si `next` existe.
    fn next_cursor(self) -> Option<String> {
        self.next?;
        self.meta?.next_cursor.filter(|c| !c.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "4ba30155-d807-446f-9e53-b67520ccfe83";

    #[test]
    fn parses_a_page() {
        let json = format!(
            r#"{{"data":[
                {{"id":"156749595","type":"tracks","meta":{{"addedAt":"2026-11-05T22:36:44.249097Z"}}}},
                {{"id":"574131217","type":"albums","meta":{{"addedAt":"2026-11-05T12:48:04.170046Z"}}}},
                {{"id":"12979","type":"artists"}},
                {{"id":"{UUID}","type":"playlists"}},
                {{"id":"../ids","type":"playlists"}},
                {{"id":"42","type":"videos"}},
                {{"id":"abc","type":"tracks"}}
            ],
            "links":{{"self":"/x","next":"/x?page%5Bcursor%5D=20","meta":{{"nextCursor":"20"}}}}}}"#
        );
        let page: RawItems = serde_json::from_str(&json).unwrap();
        let cursor = page.links.and_then(RawLinks::next_cursor);
        let entries: Vec<LaterEntry> = page.data.into_iter().filter_map(parse_entry).collect();
        assert_eq!(cursor.as_deref(), Some("20"));
        let targets: Vec<&LaterRef> = entries.iter().map(|e| &e.target).collect();
        assert_eq!(
            targets,
            [
                &LaterRef::Track(TrackId(156_749_595)),
                &LaterRef::Album(574_131_217),
                &LaterRef::Artist(12_979),
                &LaterRef::Playlist(UUID.to_owned()),
            ]
        );
        assert_eq!(entries[0].added_at, Some(1_793_918_204_249));
        assert_eq!(entries[2].added_at, None);
    }

    #[test]
    fn last_page_has_no_cursor() {
        let page: RawItems = serde_json::from_str(r#"{"data":[],"links":{"self":"/x"}}"#).unwrap();
        assert_eq!(page.links.and_then(RawLinks::next_cursor), None);
        let page: RawItems = serde_json::from_str(r#"{"data":[]}"#).unwrap();
        assert!(page.links.is_none());
    }

    fn album(id: u64) -> AlbumSummary {
        AlbumSummary {
            id,
            title: format!("album {id}"),
            cover_url: None,
            cover_large_url: None,
            artists: vec![],
            release_date: None,
            number_of_tracks: None,
            album_type: None,
            explicit: false,
            hires: false,
            color: None,
        }
    }

    #[test]
    fn assemble_keeps_order_and_skips_missing() {
        let entries = vec![
            LaterEntry {
                target: LaterRef::Album(2),
                added_at: Some(10),
            },
            LaterEntry {
                target: LaterRef::Album(1),
                added_at: Some(5),
            },
            LaterEntry {
                target: LaterRef::Artist(9),
                added_at: None,
            },
        ];
        let albums = HashMap::from([(1, album(1)), (2, album(2))]);
        let items = assemble(
            &entries,
            HashMap::new(),
            albums,
            HashMap::new(),
            HashMap::new(),
        );
        assert_eq!(
            items,
            [LaterItem::Album(album(2)), LaterItem::Album(album(1))]
        );
        let ids = ids_of(&entries);
        assert_eq!(ids.albums, [2, 1]);
        assert_eq!(ids.artists, [9]);
    }

    #[test]
    fn gone_items_are_skipped_but_not_session_errors() {
        let api = |status| TidalError::Api {
            status,
            message: String::new(),
        };
        assert!(matches!(gone_as_none::<()>(Err(api(404))), Ok(None)));
        assert!(gone_as_none::<()>(Err(api(401))).is_err());
        assert!(gone_as_none::<()>(Err(api(500))).is_err());
        assert!(gone_as_none::<()>(Err(TidalError::NotLoggedIn)).is_err());
    }
}
