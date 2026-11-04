//! ETag des playlists sur l'API réelle (#152), ignoré par défaut.
//!
//! `cargo test live_playlist_etag -- --ignored --nocapture` : crée une playlist jetable,
//! la modifie puis la supprime (aucune autre playlist n'est touchée). Vérifie ce sur quoi
//! repose le cache des pages de playlist : un GET simple renvoie l'ETag, stable tant que
//! rien ne change, différent après chaque modification ; puis `playlist_since` sur la
//! playlist jetable (inchangée → `None`, après un ajout → nouveaux titres).

use std::time::Instant;

use super::TidalClient;
use super::api::{API_V1, Call};
use super::live_tests::{ALBUM_ID, client};

/// ETag et `lastUpdated` du GET `playlists/{uuid}`.
async fn probe(client: &TidalClient, uuid: &str) -> (Option<String>, Option<serde_json::Value>) {
    let path = format!("playlists/{uuid}");
    let resp = client.call(&Call::get(API_V1, &path, &[])).await.unwrap();
    let etag = resp
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let body: serde_json::Value = resp.json().await.unwrap();
    (etag, body.get("lastUpdated").cloned())
}

#[tokio::test]
#[ignore = "réseau et session TIDAL requis, crée puis supprime une playlist"]
async fn live_playlist_etag() {
    let client = client().await;

    // Lecture seule : une playlist d'un autre utilisateur, lue deux fois.
    let search = client.search("interstellar", 10).await.unwrap();
    let other = &search
        .playlists
        .first()
        .expect("aucune playlist trouvée")
        .uuid;
    let (a, a_updated) = probe(&client, other).await;
    let (b, _) = probe(&client, other).await;
    println!("autre playlist {other} : ETag {a:?} puis {b:?}, lastUpdated {a_updated:?}");
    assert!(
        a.is_some(),
        "pas d'ETag sur la playlist d'un autre utilisateur"
    );
    assert_eq!(a, b, "ETag instable sans modification");

    // Durées : lecture complète puis revalidation de la plus grande playlist du compte.
    let library = client.library_playlists().await.unwrap().items;
    if let Some(big) = library.iter().max_by_key(|p| p.number_of_tracks) {
        let start = Instant::now();
        let full = client.playlist(&big.uuid).await.unwrap();
        let full_time = start.elapsed();
        let start = Instant::now();
        let same = client
            .playlist_since(&big.uuid, full.revision.as_deref())
            .await;
        println!(
            "« {} » ({} titres) : lecture complète {full_time:?}, revalidation {:?} ({:?})",
            big.title,
            full.tracks.len(),
            start.elapsed(),
            same.as_ref().map(Option::is_none)
        );
        assert!(matches!(same, Ok(None)));
    }

    let album = client.album(ALBUM_ID).await.unwrap();
    let ids: Vec<_> = album.tracks.iter().take(2).map(|t| t.id).collect();
    let uuid = client
        .create_playlist(
            "Seaglass test (à supprimer)",
            "Test de l'ETag, supprimée aussitôt",
        )
        .await
        .unwrap();
    let mut etags = Vec::new();
    let result = async {
        etags.push(probe(&client, &uuid).await);
        // Cache des pages : rien de changé depuis la révision affichée → `None`.
        let first = client.playlist(&uuid).await.map_err(|e| e.to_string())?;
        let known = first.revision.clone();
        let same = client.playlist_since(&uuid, known.as_deref()).await;
        println!(
            "révision {known:?}, relue sans changement : {:?}",
            same.as_ref().map(Option::is_none)
        );
        if !matches!(same, Ok(None)) {
            return Err(format!("playlist inchangée relue : {same:?}"));
        }
        client
            .add_to_playlist(&uuid, &ids)
            .await
            .map_err(|e| e.to_string())?;
        etags.push(probe(&client, &uuid).await);
        let changed = client
            .playlist_since(&uuid, known.as_deref())
            .await
            .map_err(|e| e.to_string())?
            .ok_or("ajout non vu par la révision")?;
        let got: Vec<_> = changed.tracks.iter().map(|e| e.track.id).collect();
        if got != ids || changed.revision == known {
            return Err(format!(
                "après ajout : {got:?}, révision {:?}",
                changed.revision
            ));
        }
        if client.cached_playlist(&uuid).map(|p| p.revision) != Some(changed.revision) {
            return Err("copie gardée différente de la dernière lecture".into());
        }
        client
            .remove_from_playlist(&uuid, 0, ids[0])
            .await
            .map_err(|e| e.to_string())?;
        etags.push(probe(&client, &uuid).await);
        Ok::<_, String>(())
    }
    .await;
    let deleted = client.delete_playlist(&uuid).await;
    println!("playlist jetable : {etags:?}");
    result.unwrap();
    deleted.unwrap();
    let tags: Vec<_> = etags
        .iter()
        .map(|(e, _)| e.clone().expect("pas d'ETag"))
        .collect();
    assert_ne!(tags[0], tags[1], "ETag inchangé après un ajout");
    assert_ne!(tags[1], tags[2], "ETag inchangé après un retrait");
}
