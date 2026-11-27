//! Playlist ETags on the real API (#152), ignored by default.
//!
//! `cargo test live_playlist_etag -- --ignored --nocapture`: creates a throwaway playlist,
//! modifies it then deletes it (no other playlist is touched). Checks what the playlist
//! page cache relies on: a plain GET returns the ETag, stable as long as nothing changes,
//! different after each modification; then `playlist_since` on the throwaway playlist
//! (unchanged → `None`, after an add → new tracks).

use std::time::Instant;

use super::TidalClient;
use super::api::{API_V1, Call};
use super::live_tests::{ALBUM_ID, client};

/// ETag and `lastUpdated` of the GET `playlists/{uuid}`.
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
#[ignore = "network and TIDAL session required, creates then deletes a playlist"]
async fn live_playlist_etag() {
    let client = client().await;

    // Read only: another user's playlist, read twice.
    let search = client.search("interstellar", 10).await.unwrap();
    let other = &search.playlists.first().expect("no playlist found").uuid;
    let (a, a_updated) = probe(&client, other).await;
    let (b, _) = probe(&client, other).await;
    println!("other playlist {other}: ETag {a:?} then {b:?}, lastUpdated {a_updated:?}");
    assert!(a.is_some(), "no ETag on another user's playlist");
    assert_eq!(a, b, "unstable ETag without modification");

    // Timings: full read then revalidation of the account's largest playlist.
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
            "\"{}\" ({} tracks): full read {full_time:?}, revalidation {:?} ({:?})",
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
        // Page cache: nothing changed since the displayed revision → `None`.
        let first = client.playlist(&uuid).await.map_err(|e| e.to_string())?;
        let known = first.revision.clone();
        let same = client.playlist_since(&uuid, known.as_deref()).await;
        println!(
            "revision {known:?}, read again without change: {:?}",
            same.as_ref().map(Option::is_none)
        );
        if !matches!(same, Ok(None)) {
            return Err(format!("unchanged playlist read again: {same:?}"));
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
            .ok_or("add not seen by the revision")?;
        let got: Vec<_> = changed.tracks.iter().map(|e| e.track.id).collect();
        if got != ids || changed.revision == known {
            return Err(format!(
                "after add: {got:?}, revision {:?}",
                changed.revision
            ));
        }
        if client.cached_playlist(&uuid).map(|p| p.revision) != Some(changed.revision) {
            return Err("kept copy differs from the last read".into());
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
    println!("throwaway playlist: {etags:?}");
    result.unwrap();
    deleted.unwrap();
    let tags: Vec<_> = etags
        .iter()
        .map(|(e, _)| e.clone().expect("no ETag"))
        .collect();
    assert_ne!(tags[0], tags[1], "ETag unchanged after an add");
    assert_ne!(tags[1], tags[2], "ETag unchanged after a removal");
}
