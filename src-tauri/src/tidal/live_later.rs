//! « Plus tard » sur l'API réelle (#196), ignoré par défaut.
//!
//! `cargo test live_later -- --ignored --nocapture` : **modifie le compte**. Un titre et un
//! album absents de la liste y sont ajoutés, relus puis retirés ; rien de ce qui y était
//! déjà n'est touché, et le retrait a lieu même si une vérification échoue.

use super::live_tests::{ALBUM_ID, client};
use super::models::{LaterItem, LikeTarget};

#[tokio::test]
#[ignore = "réseau et session TIDAL requis, modifie « Plus tard »"]
async fn live_later() {
    let client = client().await;
    let before = client.save_for_later().await.unwrap();
    println!(
        "« Plus tard » : {} éléments (coupée : {})",
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
        .expect("tous les titres de l'album sont déjà dans « Plus tard »");
    let mut targets = vec![LikeTarget::Track { id: track }];
    if !ids.albums.contains(&ALBUM_ID) {
        targets.push(LikeTarget::Album { id: ALBUM_ID });
    }

    for target in &targets {
        client.set_saved_for_later(target, true).await.unwrap();
    }
    // Doublon : accepté sans erreur.
    let twice = client.set_saved_for_later(&targets[0], true).await;
    let start = std::time::Instant::now();
    let after = client.save_for_later().await;
    println!("relue en {:?}", start.elapsed());
    for target in &targets {
        let removed = client.set_saved_for_later(target, false).await;
        println!("retrait de {target:?} : {removed:?}");
    }
    let removed_twice = client.set_saved_for_later(&targets[0], false).await;
    let end = client.save_for_later().await.unwrap();

    assert!(twice.is_ok(), "doublon refusé : {twice:?}");
    assert!(
        removed_twice.is_ok(),
        "second retrait refusé : {removed_twice:?}"
    );
    let after = after.unwrap();
    // Plus récent en tête : les éléments ajoutés passent devant ceux d'avant.
    let head: Vec<String> = after
        .items
        .iter()
        .take(targets.len())
        .map(|i| match i {
            LaterItem::Track(t) => format!("titre {}", t.track.id),
            LaterItem::Album(a) => format!("album {}", a.id),
            LaterItem::Artist(a) => format!("artiste {}", a.id),
            LaterItem::Playlist(p) => format!("playlist {}", p.uuid),
        })
        .collect();
    println!("en tête après ajout : {head:?}");
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
