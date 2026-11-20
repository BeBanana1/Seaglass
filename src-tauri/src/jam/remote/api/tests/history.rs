//! Historique d'écoute (#430) servi pour de vrai : route, droits, et client du téléphone.

use super::*;
use crate::handoff::history::sync;
use crate::store::{Play, Store};

fn listen(store: &Store, id: u64, at: i64) {
    store
        .add_play(&Play {
            track: &track(id),
            album: &album(id * 10),
            source: None,
            played_at: at,
            format: None,
        })
        .unwrap();
}

fn tracks(store: &Store) -> Vec<u64> {
    let mut ids: Vec<u64> = store
        .played_track_ids()
        .unwrap()
        .into_iter()
        .map(|id| id.0)
        .collect();
    ids.sort_unstable();
    ids
}

#[tokio::test]
async fn history_needs_transfers_and_a_valid_body() {
    let (addr, site) = start().await;
    let body =
        r#"{"device":"0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d","since":0,"plays":{"plays":[]}}"#;
    assert_eq!(post(addr, "/api/plays/sync", body).await.0, 403);
    site.owner.set_features(false, true);
    assert_eq!(post(addr, "/api/plays/sync", body).await.0, 200);
    assert_eq!(post(addr, "/api/plays/sync", "{}").await.0, 400);
    let bad = body.replace("0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d", "pas-un-appareil");
    assert_eq!(post(addr, "/api/plays/sync", &bad).await.0, 400);
}

/// Le téléphone et le PC finissent avec le même historique, en plusieurs lots, et une
/// seconde synchro n'échange plus rien.
#[tokio::test]
async fn the_phone_and_the_computer_converge() {
    let (addr, site) = start().await;
    site.owner.set_features(false, true);
    let pc = Arc::clone(site.remote.history());
    // Plus d'un lot dans chaque sens.
    for i in 0..1_200 {
        listen(&pc, i, 1_000 + i as i64);
    }
    let phone = Arc::new(Store::in_memory().unwrap());
    for i in 5_000..6_100 {
        listen(&phone, i, 2_000 + i as i64);
    }
    // Une écoute commune (même titre, même instant) ne doit pas être doublée.
    listen(&phone, 3, 1_003);

    let link = crate::handoff::Link::parse(&format!("http://{addr}/#AB23CDEF45GH")).unwrap();
    assert_eq!(sync(&link, &phone).await.unwrap(), 1_199);
    assert_eq!(tracks(&pc), tracks(&phone));
    assert_eq!(tracks(&pc).len(), 2_300);
    let state = phone.sync_state().unwrap();
    assert_eq!(state.peer, Some(pc.device_id().unwrap()));
    assert!(state.synced_at.is_some());

    // Rien de neuf : un seul échange, rien de reçu.
    assert_eq!(sync(&link, &phone).await.unwrap(), 0);
    // Une nouvelle écoute de chaque côté passe à la synchro suivante.
    listen(&pc, 9_001, 9_000_000);
    listen(&phone, 9_002, 9_000_001);
    assert_eq!(sync(&link, &phone).await.unwrap(), 1);
    assert_eq!(tracks(&pc), tracks(&phone));

    // Effacement local : rien ne revient de l'autre côté.
    phone.clear_plays().unwrap();
    assert_eq!(sync(&link, &phone).await.unwrap(), 0);
    assert!(tracks(&phone).is_empty());
    assert_eq!(tracks(&pc).len(), 2_302);
}
