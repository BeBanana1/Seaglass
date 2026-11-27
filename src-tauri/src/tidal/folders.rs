//! Playlist folders (#331), checked on 2026-11-12 (`cargo test live_folders -- --ignored
//! --nocapture`, throwaway folders and playlists).
//!
//! - List: GET v2 `my-collection/playlists/folders?folderId=root|<id>` (`includeOnly=FOLDER`
//!   for folders only), and `…/folders/flattened`: folders and all the playlists,
//!   each with its `parent`. Paging by `cursor`.
//! - Create: PUT `…/folders/create-folder?name=&folderId=root&trns=` (playlists filed
//!   right away). One level only: a `folderId` other than `root` is ignored.
//! - Rename: PUT `…/folders/rename?trn=trn:folder:<id>&name=` (empty name: 400).
//! - Move: PUT `…/folders/move?folderId=<id>|root&trns=trn:playlist:a,trn:playlist:b`.
//! - Delete: PUT `…/folders/remove?trns=trn:folder:<id>`. The folder's playlists are not
//!   deleted but stay orphaned (missing from the root): they are moved back there first.

use reqwest::Method;
use serde::Deserialize;

use super::api::API_V2;
use super::models::PlaylistFolder;
use super::{Result, TidalClient, TidalError};

const FOLDERS: &str = "my-collection/playlists/folders";
const PAGE_SIZE: usize = 50;
/// Pages read at most (2,500 items): safeguard against a cursor that would never end.
const MAX_PAGES: usize = 50;
/// Playlists moved per request.
const MOVE_CHUNK: usize = 50;

impl TidalClient {
    /// The user's folders, sorted by name, with the playlists they contain.
    /// Without folders, a single small request (`includeOnly=FOLDER`).
    pub async fn playlist_folders(&self) -> Result<Vec<PlaylistFolder>> {
        let root = [
            ("folderId", "root".to_owned()),
            ("includeOnly", "FOLDER".to_owned()),
        ];
        if self.folder_items(FOLDERS, &root).await?.is_empty() {
            return Ok(Vec::new());
        }
        let all = self
            .folder_items(&format!("{FOLDERS}/flattened"), &[])
            .await?;
        Ok(group(all))
    }

    /// Creates a folder at the root, with these playlists in it.
    pub async fn create_folder(&self, name: &str, uuids: &[String]) -> Result<PlaylistFolder> {
        let mut query = vec![("name", name.to_owned()), ("folderId", "root".to_owned())];
        if !uuids.is_empty() {
            query.push(("trns", playlist_trns(uuids)));
        }
        let path = format!("{FOLDERS}/create-folder");
        let created: RawFolderItem = self
            .write(Method::PUT, API_V2, &path, &query, None, None)
            .await?
            .json()
            .await
            .map_err(|e| TidalError::Decode(format!("{path} : {e}")))?;
        let id = created
            .data
            .id
            .ok_or_else(|| TidalError::Decode(format!("{path}: missing id")))?;
        Ok(PlaylistFolder {
            id,
            name: created.name.unwrap_or_else(|| name.to_owned()),
            playlists: uuids.to_vec(),
        })
    }

    pub async fn rename_folder(&self, id: &str, name: &str) -> Result<()> {
        let query = [
            ("trn", format!("trn:folder:{id}")),
            ("name", name.to_owned()),
        ];
        self.write(
            Method::PUT,
            API_V2,
            &format!("{FOLDERS}/rename"),
            &query,
            None,
            None,
        )
        .await
        .map(drop)
    }

    /// Files playlists into a folder, or moves them back to the root (`None`).
    pub async fn move_to_folder(&self, uuids: &[String], folder: Option<&str>) -> Result<()> {
        let path = format!("{FOLDERS}/move");
        for chunk in uuids.chunks(MOVE_CHUNK) {
            let query = [
                ("folderId", folder.unwrap_or("root").to_owned()),
                ("trns", playlist_trns(chunk)),
            ];
            self.write(Method::PUT, API_V2, &path, &query, None, None)
                .await?;
        }
        Ok(())
    }

    /// Deletes a folder while keeping its playlists, moved back to the root first.
    pub async fn delete_folder(&self, id: &str) -> Result<()> {
        let query = [("folderId", id.to_owned())];
        let inside: Vec<String> = self
            .folder_items(FOLDERS, &query)
            .await?
            .into_iter()
            .filter_map(RawFolderItem::playlist_uuid)
            .collect();
        self.move_to_folder(&inside, None).await?;
        let query = [("trns", format!("trn:folder:{id}"))];
        self.write(
            Method::PUT,
            API_V2,
            &format!("{FOLDERS}/remove"),
            &query,
            None,
            None,
        )
        .await
        .map(drop)
    }

    /// All the items of a folder list, page after page.
    async fn folder_items(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<Vec<RawFolderItem>> {
        let mut items = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let mut query = params.to_vec();
            query.push(("limit", PAGE_SIZE.to_string()));
            if let Some(c) = cursor.take() {
                query.push(("cursor", c));
            }
            let page: RawFolderPage = self.get_at(API_V2, path, &query).await?;
            let last = page.items.is_empty();
            items.extend(page.items);
            match page.cursor {
                Some(c) if !last => cursor = Some(c),
                _ => return Ok(items),
            }
        }
        tracing::warn!("{path}: more than {MAX_PAGES} pages, list truncated");
        Ok(items)
    }
}

fn playlist_trns(uuids: &[String]) -> String {
    uuids
        .iter()
        .map(|u| format!("trn:playlist:{u}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// Groups the items of `flattened`: each folder with its playlists, in the
/// API order; folders sorted by name.
fn group(items: Vec<RawFolderItem>) -> Vec<PlaylistFolder> {
    let mut folders: Vec<PlaylistFolder> = items
        .iter()
        .filter(|i| i.item_type == "FOLDER")
        .filter_map(|i| {
            Some(PlaylistFolder {
                id: i.data.id.clone()?,
                name: i.name.clone().unwrap_or_default(),
                playlists: Vec::new(),
            })
        })
        .collect();
    for item in items {
        let Some(parent) = item.parent.as_ref().and_then(|p| p.id.clone()) else {
            continue;
        };
        let Some(folder) = folders.iter_mut().find(|f| f.id == parent) else {
            continue;
        };
        if let Some(uuid) = item.playlist_uuid() {
            folder.playlists.push(uuid);
        }
    }
    folders.sort_by_cached_key(|f| f.name.to_lowercase());
    folders
}

// --- Raw JSON formats ----------------------------------------------------------------

#[derive(Deserialize)]
struct RawFolderPage {
    #[serde(default)]
    items: Vec<RawFolderItem>,
    cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFolderItem {
    #[serde(default)]
    item_type: String,
    name: Option<String>,
    trn: Option<String>,
    parent: Option<RawParent>,
    #[serde(default)]
    data: RawItemData,
}

#[derive(Deserialize)]
struct RawParent {
    id: Option<String>,
}

#[derive(Default, Deserialize)]
struct RawItemData {
    id: Option<String>,
    uuid: Option<String>,
}

impl RawFolderItem {
    fn playlist_uuid(self) -> Option<String> {
        if self.item_type != "PLAYLIST" {
            return None;
        }
        self.data.uuid.or_else(|| {
            self.trn
                .and_then(|t| t.strip_prefix("trn:playlist:").map(str::to_owned))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(json: &str) -> Vec<RawFolderItem> {
        serde_json::from_str::<RawFolderPage>(json).unwrap().items
    }

    /// Shape of `flattened` seen on 2026-11-12 (reduced fields).
    const FLATTENED: &str = r#"{"cursor": null, "items": [
        {"itemType": "FOLDER", "name": "rock", "parent": null, "trn": "trn:folder:f2",
         "data": {"id": "f2", "itemType": "FOLDER", "totalNumberOfItems": 1}},
        {"itemType": "FOLDER", "name": "Jazz", "parent": null, "trn": "trn:folder:f1",
         "data": {"id": "f1", "itemType": "FOLDER", "totalNumberOfItems": 2}},
        {"itemType": "PLAYLIST", "name": "Root", "parent": null, "trn": "trn:playlist:p0",
         "data": {"uuid": "p0", "title": "Root"}},
        {"itemType": "PLAYLIST", "name": "B", "trn": "trn:playlist:p2",
         "parent": {"atRoot": false, "id": "f1", "name": "Jazz"}, "data": {"uuid": "p2"}},
        {"itemType": "PLAYLIST", "name": "A", "trn": "trn:playlist:p1",
         "parent": {"id": "f1"}, "data": {}},
        {"itemType": "PLAYLIST", "name": "C", "trn": "trn:playlist:p3",
         "parent": {"id": "f2"}, "data": {"uuid": "p3"}},
        {"itemType": "PLAYLIST", "name": "Orpheline", "trn": "trn:playlist:p4",
         "parent": {"id": "gone"}, "data": {"uuid": "p4"}}
    ]}"#;

    #[test]
    fn groups_playlists_by_folder_sorted_by_name() {
        let folders = group(items(FLATTENED));
        assert_eq!(
            folders,
            vec![
                PlaylistFolder {
                    id: "f1".into(),
                    name: "Jazz".into(),
                    playlists: vec!["p2".into(), "p1".into()],
                },
                PlaylistFolder {
                    id: "f2".into(),
                    name: "rock".into(),
                    playlists: vec!["p3".into()],
                },
            ]
        );
    }

    #[test]
    fn no_folder_no_group() {
        assert!(group(items(r#"{"items": [], "cursor": null}"#)).is_empty());
    }

    #[test]
    fn created_folder_response() {
        let created: RawFolderItem = serde_json::from_str(
            r#"{"addedAt": "2026-11-12T18:35:33.232+0000", "itemType": "FOLDER",
                "name": "Road trip", "parent": null, "trn": "trn:folder:abc",
                "data": {"id": "abc", "name": "Road trip", "totalNumberOfItems": 0}}"#,
        )
        .unwrap();
        assert_eq!(created.data.id.as_deref(), Some("abc"));
        assert_eq!(created.playlist_uuid(), None);
    }

    #[test]
    fn trns_are_joined() {
        assert_eq!(
            playlist_trns(&["a".into(), "b".into()]),
            "trn:playlist:a,trn:playlist:b"
        );
    }
}
