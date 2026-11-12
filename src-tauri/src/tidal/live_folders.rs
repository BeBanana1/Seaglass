//! Dossiers de playlists (#331), sur l'API réelle, ignorés par défaut.
//!
//! `cargo test live_folders -- --ignored --nocapture` : **crée deux playlists et un dossier
//! jetables**, les range, renomme, supprime le dossier (les playlists doivent revenir à la
//! racine), puis supprime les playlists. Nettoyage même si une vérification échoue.

use super::TidalClient;
use super::api::{API_V2, Call};
use super::live_tests::client;

/// Uuids des playlists à la racine (hors dossier).
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
    println!("dossier créé : {folder:?}");
    let result = async {
        let found = client.playlist_folders().await.map_err(e)?;
        let mine = found.iter().find(|f| f.id == folder.id);
        println!("lu : {mine:?}");
        if mine.map(|f| f.playlists.clone()) != Some(vec![a.to_owned()]) {
            return Err("A absente du dossier lu".to_owned());
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
        println!("après rangement et renommage : {mine:?}");
        let mine = mine.ok_or("dossier introuvable")?;
        if mine.name != "Seaglass test renommé" || mine.playlists.len() != 2 {
            return Err("renommage ou rangement non vu".to_owned());
        }
        let root = root_playlists(client).await;
        if root.iter().any(|u| u == a || u == b) {
            return Err("playlist rangée encore à la racine".to_owned());
        }
        Ok(())
    }
    .await;
    // Toujours : suppression du dossier, qui doit ressortir A et B à la racine.
    client.delete_folder(&folder.id).await.map_err(e)?;
    let left = client.playlist_folders().await.map_err(e)?;
    if left.iter().any(|f| f.id == folder.id) {
        return Err("dossier encore là".to_owned());
    }
    let root = root_playlists(client).await;
    let (has_a, has_b) = (root.iter().any(|u| u == a), root.iter().any(|u| u == b));
    println!("après suppression du dossier : A à la racine {has_a}, B {has_b}");
    if !(has_a && has_b) {
        return Err("playlists non revenues à la racine".to_owned());
    }
    result
}

#[tokio::test]
#[ignore = "réseau et session TIDAL requis, crée puis supprime un dossier et deux playlists"]
async fn live_folders() {
    let client = client().await;
    println!("dossiers existants : {:?}", client.playlist_folders().await);
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
    println!("playlists supprimées : {deleted:?}");
    result.unwrap();
    assert!(deleted.0.is_ok() && deleted.1.is_ok());
}
