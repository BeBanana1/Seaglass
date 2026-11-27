//! Blocks on the real API (#334), ignored by default.
//!
//! `cargo test live_blocks -- --ignored --nocapture`: **modifies the account**. Blocks then
//! unblocks a track and an artist that are not in the lists; nothing that was already
//! blocked is touched, and the unblock happens even if a check fails.

use super::TidalClient;
use super::live_tests::{ALBUM_ID, client};
use super::models::{BlockTarget, Blocked, Track};

fn contains(blocked: &Blocked, target: &BlockTarget) -> bool {
    match target {
        BlockTarget::Track { id } => blocked.tracks.iter().any(|t| t.id == *id),
        BlockTarget::Artist { id } => blocked.artists.iter().any(|a| a.id == *id),
    }
}

/// Is a track of the test album kept as is in a list filtered by the blocks?
async fn kept(client: &TidalClient, track: &Track) -> bool {
    client
        .without_blocked(vec![track.clone()])
        .await
        .iter()
        .any(|t| t.id == track.id)
}

#[tokio::test]
#[ignore = "network and TIDAL session required, modifies the blocks"]
async fn live_blocks() {
    let client = client().await;
    let before = client.blocked().await.unwrap();
    println!(
        "blocked: {} tracks, {} artists",
        before.tracks.len(),
        before.artists.len()
    );
    let album = client.album(ALBUM_ID).await.unwrap();
    let track = album.tracks[0].clone();
    let targets = [
        BlockTarget::Track { id: track.id },
        BlockTarget::Artist {
            id: track.artists[0].id,
        },
    ];
    let mut failures = Vec::new();
    for target in &targets {
        if contains(&before, target) {
            println!("{target:?} already blocked, skipped");
            continue;
        }
        let blocked = client.set_blocked(target, true).await;
        let listed = contains(&client.blocked().await.unwrap(), target);
        let filtered = !kept(&client, &track).await;
        // Second block: idempotent (200 seen on 2026-11-12).
        let twice = client.set_blocked(target, true).await;
        // Always unblock: the target was not blocked before the test.
        let unblocked = client.set_blocked(target, false).await;
        let after = client.blocked().await.unwrap();
        let back = kept(&client, &track).await;
        println!(
            "{target:?}: block {blocked:?}, listed {listed}, filtered {filtered}, second {twice:?}, \
             unblock {unblocked:?}, restored {back}"
        );
        if blocked.is_err() || !listed || !filtered || twice.is_err() || unblocked.is_err() {
            failures.push(format!("{target:?}"));
        }
        if contains(&after, target) || !back {
            failures.push(format!("{target:?} still blocked"));
        }
    }
    // Unblocking what is no longer blocked: no error either (204).
    if !contains(&client.blocked().await.unwrap(), &targets[0]) {
        assert!(client.set_blocked(&targets[0], false).await.is_ok());
    }
    assert!(failures.is_empty(), "{failures:?}");
}
