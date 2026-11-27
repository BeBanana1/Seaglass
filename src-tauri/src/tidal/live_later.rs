//! "Save for later" on the real API (#196), ignored by default.
//!
//! `cargo test live_later -- --ignored --nocapture`: **modifies the account**. A track and
//! an album absent from the list are added to it, read again then removed; nothing that
//! was already there is touched, and the removal happens even if a check fails.

use super::live_tests::{ALBUM_ID, client};
use super::models::{LaterItem, LikeTarget};

#[tokio::test]
#[ignore = "network and TIDAL session required, modifies \"Save for later\""]
async fn live_later() {
    let client = client().await;
    let before = client.save_for_later().await.unwrap();
    println!(
        "\"Save for later\": {} items (truncated: {})",
        before.items.len(),
        before.truncated
    );
    let ids = client.later_ids().await.unwrap();
    let album = client.album(ALBUM_ID).await.unwrap();
    let track = album
        .tracks
        .iter()
        .map(|t| t.id)
        .find(|id| !ids.tracks.contains(id))
        .expect("all the album's tracks are already in \"Save for later\"");
    let mut targets = vec![LikeTarget::Track { id: track }];
    if !ids.albums.contains(&ALBUM_ID) {
        targets.push(LikeTarget::Album { id: ALBUM_ID });
    }

    for target in &targets {
        client.set_saved_for_later(target, true).await.unwrap();
    }
    // Duplicate: accepted without an error.
    let twice = client.set_saved_for_later(&targets[0], true).await;
    let start = std::time::Instant::now();
    let after = client.save_for_later().await;
    println!("read again in {:?}", start.elapsed());
    for target in &targets {
        let removed = client.set_saved_for_later(target, false).await;
        println!("removal of {target:?}: {removed:?}");
    }
    let removed_twice = client.set_saved_for_later(&targets[0], false).await;
    let end = client.save_for_later().await.unwrap();

    assert!(twice.is_ok(), "duplicate refused: {twice:?}");
    assert!(
        removed_twice.is_ok(),
        "second removal refused: {removed_twice:?}"
    );
    let after = after.unwrap();
    // Most recent first: the added items come before the earlier ones.
    let head: Vec<String> = after
        .items
        .iter()
        .take(targets.len())
        .map(|i| match i {
            LaterItem::Track(t) => format!("track {}", t.track.id),
            LaterItem::Album(a) => format!("album {}", a.id),
            LaterItem::Artist(a) => format!("artist {}", a.id),
            LaterItem::Playlist(p) => format!("playlist {}", p.uuid),
        })
        .collect();
    println!("at the head after adding: {head:?}");
    assert!(
        after.items.iter().any(
            |i| matches!(i, LaterItem::Track(t) if t.track.id == track && t.added_at.is_some())
        )
    );
    assert_eq!(after.items.len(), before.items.len() + targets.len());
    assert_eq!(end.items, before.items);
    let ids = client.later_ids().await.unwrap();
    assert!(!ids.tracks.contains(&track));
}
