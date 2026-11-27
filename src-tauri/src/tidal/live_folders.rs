//! Playlist folders (#331), on the real API, ignored by default.
//!
//! `cargo test live_folders -- --ignored --nocapture`: **creates two throwaway playlists
//! and a folder**, files them, renames, deletes the folder (the playlists must go back to
//! the root), then deletes the playlists. Cleanup even if a check fails.

use super::TidalClient;
use super::api::{API_V2, Call};
use super::live_tests::client;

/// Uuids of the playlists at the root (outside any folder).
async fn root_playlists(client: &TidalClient) -> Vec<String> {
    let query = [
        ("folderId", "root".to_owned()),
        ("includeOnly", "PLAYLIST".to_owned()),
        ("limit", "50".to_owned()),
    ];
    let call = Call::get(API_V2, "my-collection/playlists/folders", &query);
    let body: serde_json::Value = client.call(&call).await.unwrap().json().await.unwrap();
    body["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|i| i["data"]["uuid"].as_str().map(str::to_owned))
        .collect()
}

async fn checks(client: &TidalClient, a: &str, b: &str) -> Result<(), String> {
    let e = |e: super::TidalError| e.to_string();
    let folder = client
        .create_folder("Seaglass test (à supprimer)", &[a.to_owned()])
        .await
        .map_err(e)?;
    println!("folder created: {folder:?}");
    let result = async {
        let found = client.playlist_folders().await.map_err(e)?;
        let mine = found.iter().find(|f| f.id == folder.id);
        println!("read: {mine:?}");
        if mine.map(|f| f.playlists.clone()) != Some(vec![a.to_owned()]) {
            return Err("A missing from the folder read".to_owned());
        }
        client
            .move_to_folder(&[b.to_owned()], Some(&folder.id))
            .await
            .map_err(e)?;
        client
            .rename_folder(&folder.id, "Seaglass test renommé")
            .await
            .map_err(e)?;
        let found = client.playlist_folders().await.map_err(e)?;
        let mine = found.iter().find(|f| f.id == folder.id).cloned();
        println!("after filing and renaming: {mine:?}");
        let mine = mine.ok_or("folder not found")?;
        if mine.name != "Seaglass test renommé" || mine.playlists.len() != 2 {
            return Err("rename or filing not seen".to_owned());
        }
        let root = root_playlists(client).await;
        if root.iter().any(|u| u == a || u == b) {
            return Err("filed playlist still at the root".to_owned());
        }
        Ok(())
    }
    .await;
    // Always: delete the folder, which must put A and B back at the root.
    client.delete_folder(&folder.id).await.map_err(e)?;
    let left = client.playlist_folders().await.map_err(e)?;
    if left.iter().any(|f| f.id == folder.id) {
        return Err("folder still there".to_owned());
    }
    let root = root_playlists(client).await;
    let (has_a, has_b) = (root.iter().any(|u| u == a), root.iter().any(|u| u == b));
    println!("after deleting the folder: A at the root {has_a}, B {has_b}");
    if !(has_a && has_b) {
        return Err("playlists not back at the root".to_owned());
    }
    result
}

#[tokio::test]
#[ignore = "network and TIDAL session required, creates then deletes a folder and two playlists"]
async fn live_folders() {
    let client = client().await;
    println!("existing folders: {:?}", client.playlist_folders().await);
    let a = client
        .create_playlist("Seaglass test A (à supprimer)", "")
        .await
        .unwrap();
    let b = client
        .create_playlist("Seaglass test B (à supprimer)", "")
        .await
        .unwrap();
    let result = checks(&client, &a, &b).await;
    let deleted = (
        client.delete_playlist(&a).await,
        client.delete_playlist(&b).await,
    );
    println!("playlists deleted: {deleted:?}");
    result.unwrap();
    assert!(deleted.0.is_ok() && deleted.1.is_ok());
}
