//! Blocages sur l'API réelle (#334), ignorés par défaut.
//!
//! `cargo test live_blocks -- --ignored --nocapture` : **modifie le compte**. Bloque puis
//! débloque un titre et un artiste absents des listes ; rien de ce qui était déjà bloqué
//! n'est touché, et le déblocage a lieu même si une vérification échoue.

use super::TidalClient;
use super::live_tests::{ALBUM_ID, client};
use super::models::{BlockTarget, Blocked, Track};

fn contains(blocked: &Blocked, target: &BlockTarget) -> bool {
    match target {
        BlockTarget::Track { id } => blocked.tracks.iter().any(|t| t.id == *id),
        BlockTarget::Artist { id } => blocked.artists.iter().any(|a| a.id == *id),
    }
}

/// Un titre de l'album de test, présent tel quel dans une liste filtrée par les blocages ?
async fn kept(client: &TidalClient, track: &Track) -> bool {
    client
        .without_blocked(vec![track.clone()])
        .await
        .iter()
        .any(|t| t.id == track.id)
}

#[tokio::test]
#[ignore = "réseau et session TIDAL requis, modifie les blocages"]
async fn live_blocks() {
    let client = client().await;
    let before = client.blocked().await.unwrap();
    println!(
        "bloqués : {} titres, {} artistes",
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
            println!("{target:?} déjà bloqué, sauté");
            continue;
        }
        let blocked = client.set_blocked(target, true).await;
        let listed = contains(&client.blocked().await.unwrap(), target);
        let filtered = !kept(&client, &track).await;
        // Second blocage : idempotent (200 vu le 2026-11-12).
        let twice = client.set_blocked(target, true).await;
        // Déblocage systématique : la cible n'était pas bloquée avant le test.
        let unblocked = client.set_blocked(target, false).await;
        let after = client.blocked().await.unwrap();
        let back = kept(&client, &track).await;
        println!(
            "{target:?} : blocage {blocked:?}, listé {listed}, filtré {filtered}, second {twice:?}, \
             déblocage {unblocked:?}, rendu {back}"
        );
        if blocked.is_err() || !listed || !filtered || twice.is_err() || unblocked.is_err() {
            failures.push(format!("{target:?}"));
        }
        if contains(&after, target) || !back {
            failures.push(format!("{target:?} toujours bloqué"));
        }
    }
    // Déblocage de ce qui n'est plus bloqué : sans erreur aussi (204).
    if !contains(&client.blocked().await.unwrap(), &targets[0]) {
        assert!(client.set_blocked(&targets[0], false).await.is_ok());
    }
    assert!(failures.is_empty(), "{failures:?}");
}
