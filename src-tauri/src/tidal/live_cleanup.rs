//! List cleanup (#382), on the real API, ignored by default.
//!
//! - `cargo test live_cleanup_probe -- --ignored --nocapture`: read only. For each user
//!   playlist, compares `numberOfTracks`, `playlists/{uuid}/tracks` and
//!   `playlists/{uuid}/items`, prints the fields of unreadable tracks, then the favorites
//!   hidden by the API.
//! - `cargo test live_cleanup_report -- --ignored --nocapture`: read only, report of each
//!   user playlist and of the liked tracks.
//! - `cargo test live_cleanup_apply -- --ignored --nocapture`: **creates a throwaway
//!   playlist** (duplicate and unavailable track), cleans it, undoes, then deletes it.

use std::time::{Duration, Instant};

use super::TidalClient;
use super::api::{API_V1, Call};
use super::live_tests::{ALBUM_ID, client};
use super::models::{CleanupItem, ListOp, TrackId};

/// Unavailable track of a playlist of the test account ("Love Tonight (Robin Schulz
/// Remix)", seen on 2026-11-12), playable under another id (same ISRC).
const UNAVAILABLE: TrackId = TrackId(193_949_615);

async fn raw_all(client: &TidalClient, path: &str) -> (Option<u64>, Vec<serde_json::Value>) {
    let mut items = Vec::new();
    let mut total = None;
    let mut offset = 0u64;
    loop {
        let query = [("offset", offset.to_string()), ("limit", "100".to_owned())];
        let resp = client.call(&Call::get(API_V1, path, &query)).await.unwrap();
        let body: serde_json::Value = resp.json().await.unwrap();
        total = body["totalNumberOfItems"].as_u64().or(total);
        let page = body["items"].as_array().cloned().unwrap_or_default();
        let n = page.len();
        items.extend(page);
        offset += 100;
        if n == 0 || total.is_some_and(|t| offset >= t) {
            break;
        }
    }
    (total, items)
}

fn unavailable(t: &serde_json::Value) -> bool {
    t["streamReady"] == false || t["allowStreaming"] == false
}

#[tokio::test]
#[ignore = "network and TIDAL session required"]
async fn live_cleanup_probe() {
    let client = client().await;
    for p in &client.own_playlists().await.unwrap() {
        let (tracks_total, tracks) =
            raw_all(&client, &format!("playlists/{}/tracks", p.uuid)).await;
        let (items_total, items) = raw_all(&client, &format!("playlists/{}/items", p.uuid)).await;
        let bad: Vec<&serde_json::Value> = tracks.iter().filter(|t| unavailable(t)).collect();
        println!(
            "{}: number={:?} tracks={}/{:?} (unavailable {}) items={}/{:?}",
            p.title,
            p.number_of_tracks,
            tracks.len(),
            tracks_total,
            bad.len(),
            items.len(),
            items_total,
        );
        for t in bad.iter().take(3) {
            println!(
                "   unavailable: id={} \"{}\" streamReady={} allowStreaming={} isrc={}",
                t["id"], t["title"], t["streamReady"], t["allowStreaming"], t["isrc"]
            );
        }
    }
    let user = client.user_id().await.unwrap();
    let (total, favs) = raw_all(&client, &format!("users/{user}/favorites/tracks")).await;
    let seen: std::collections::HashSet<u64> = favs
        .iter()
        .filter_map(|f| f["item"]["id"].as_u64())
        .collect();
    println!("favorites: {}/{total:?}", favs.len());
    let ids = client.liked_ids().await.unwrap().tracks;
    for id in ids.iter().filter(|id| !seen.contains(&id.0)).take(3) {
        let resp = client
            .call(&Call::get(API_V1, &format!("tracks/{id}"), &[]))
            .await;
        println!(
            "   hidden {id}: {}",
            resp.err().map_or("readable".into(), |e| e.to_string())
        );
    }
}

fn describe(item: &CleanupItem) -> String {
    match item {
        CleanupItem::Duplicate {
            index,
            track,
            kept_index,
            same_track,
            ..
        } => format!(
            "{index}: duplicate{} of {kept_index} \"{}\"",
            if *same_track { "" } else { " (edition)" },
            track.title
        ),
        CleanupItem::Unavailable {
            index,
            track,
            replacement,
        } => format!(
            "{index}: unavailable \"{}\" → {}",
            track.title,
            replacement.as_ref().map_or("none".into(), |r| format!(
                "{} \"{}\" ({})",
                r.id, r.title, r.album.title
            ))
        ),
    }
}

#[tokio::test]
#[ignore = "network and TIDAL session required"]
async fn live_cleanup_report() {
    let client = client().await;
    for p in &client.own_playlists().await.unwrap() {
        let start = Instant::now();
        let report = client.playlist_cleanup(&p.uuid).await.unwrap();
        println!(
            "{}: {} tracks, {} changes in {:?}",
            p.title,
            report.scanned,
            report.items.len(),
            start.elapsed()
        );
        for item in &report.items {
            println!("   {} → {:?}", describe(&item.item), item.op);
        }
    }
    let start = Instant::now();
    let report = client.liked_cleanup().await.unwrap();
    println!(
        "liked tracks: {} tracks, {} changes in {:?}",
        report.scanned,
        report.items.len(),
        start.elapsed()
    );
    for item in &report.items {
        println!("   {} → {:?}", describe(&item.item), item.op);
    }
}

async fn ids(client: &TidalClient, uuid: &str) -> Vec<u64> {
    let (_, tracks) = raw_all(client, &format!("playlists/{uuid}/tracks")).await;
    tracks.iter().filter_map(|t| t["id"].as_u64()).collect()
}

/// Reads again until `expected` is seen: right after a write, the API may return the old
/// state (#201).
async fn wait_for(client: &TidalClient, uuid: &str, expected: &[u64]) -> Vec<u64> {
    let mut seen = Vec::new();
    for _ in 0..10 {
        seen = ids(client, uuid).await;
        if seen == expected {
            break;
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
    seen
}

async fn check_apply(client: &TidalClient, uuid: &str) {
    let album = client.album(ALBUM_ID).await.unwrap();
    let (a, b, c) = (album.tracks[0].id, album.tracks[1].id, album.tracks[2].id);
    client.add_to_playlist(uuid, &[a, b]).await.unwrap();
    assert!(client.insert_in_playlist(uuid, 2, a).await.unwrap());
    // Seen on 2026-11-12: an unavailable track cannot be added (404).
    let refused = client.insert_in_playlist(uuid, 3, UNAVAILABLE).await;
    println!("adding an unavailable track: {refused:?}");
    let original = wait_for(client, uuid, &[a.0, b.0, a.0]).await;
    println!("throwaway playlist: {original:?}");

    let report = client.playlist_cleanup(uuid).await.unwrap();
    for item in &report.items {
        println!("   {} → {:?}", describe(&item.item), item.op);
    }
    assert!(
        matches!(
            report
                .items
                .iter()
                .map(|c| &c.item)
                .collect::<Vec<_>>()
                .as_slice(),
            [CleanupItem::Duplicate {
                index: 2,
                kept_index: 0,
                same_track: true,
                ..
            }]
        ),
        "duplicate at position 2"
    );
    // The duplicate removed, and a replacement between readable tracks (b → c).
    let ops = vec![
        ListOp::Remove {
            index: 2,
            track_id: a,
        },
        ListOp::Replace {
            index: 1,
            track_id: b,
            with: c,
        },
    ];
    let start = Instant::now();
    let done = client.clean_playlist(uuid, ops).await.unwrap();
    println!("cleanup: {done:?} in {:?}", start.elapsed());
    assert_eq!((done.applied, done.skipped, &done.error), (2, 0, &None));
    let cleaned = wait_for(client, uuid, &[a.0, c.0]).await;
    println!("after cleanup: {cleaned:?}");
    assert_eq!(cleaned, [a.0, c.0]);

    // Undo, preceded by an unavailable track to put back at the top: it is skipped and
    // the following positions are realigned.
    let mut undo = vec![ListOp::Insert {
        index: 0,
        track_id: UNAVAILABLE,
    }];
    undo.extend(done.undo.iter().map(|op| match *op {
        ListOp::Remove { index, track_id } => ListOp::Remove {
            index: index + 1,
            track_id,
        },
        ListOp::Insert { index, track_id } => ListOp::Insert {
            index: index + 1,
            track_id,
        },
        ListOp::Replace {
            index,
            track_id,
            with,
        } => ListOp::Replace {
            index: index + 1,
            track_id,
            with,
        },
    }));
    let start = Instant::now();
    let undone = client.apply_playlist_ops(uuid, &undo).await;
    println!("undo: {undone:?} in {:?}", start.elapsed());
    let restored = wait_for(client, uuid, &original).await;
    println!("after undo: {restored:?}");
    assert_eq!(
        (undone.applied, undone.skipped, &undone.error),
        (2, 1, &None)
    );
    assert_eq!(restored, original);
}

#[tokio::test]
#[ignore = "network and TIDAL session required, creates then deletes a playlist"]
async fn live_cleanup_apply() {
    let client = client().await;
    let uuid = client
        .create_playlist("Seaglass test (à supprimer)", "Créée par un test")
        .await
        .unwrap();
    // Clean up even if the test fails.
    let result = tokio::spawn({
        let uuid = uuid.clone();
        async move {
            let client = super::live_tests::client().await;
            check_apply(&client, &uuid).await;
        }
    })
    .await;
    let deleted = client.delete_playlist(&uuid).await;
    result.unwrap();
    deleted.unwrap();
}
