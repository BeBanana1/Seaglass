//! Règles et exclusions de l'hôte (#288), appelées directement sur `Core` avec le faux
//! lecteur (le transport est testé dans `server.rs` et `relay.rs`).

use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;

use serde_json::{Value, json};

use super::Core;
use crate::jam::rules::JamRules;
use crate::jam::testing::Fake;

const CODE: &str = "AB23CD";
const LEA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const MAX: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20));
const OTHER_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 21));

fn core(rules: JamRules, ban_ips: bool) -> Core {
    Core::with_rules(CODE.into(), Arc::default(), rules, ban_ips)
}

async fn call(core: &Core, fake: &Fake, path: &str, body: Value, ip: IpAddr) -> (u16, Value) {
    let body = body.to_string();
    let response = core.api(path, Some(CODE), body.as_bytes(), ip, fake).await;
    let json = serde_json::from_slice(&response.body).unwrap_or(Value::Null);
    (response.status, json)
}

/// Recherche (les titres 1 et 2, l'album 40 deviennent ajoutables).
async fn search(core: &Core, fake: &Fake) {
    let (status, _) = call(
        core,
        fake,
        "/api/search",
        json!({ "query": "x", "albums": true }),
        IP,
    )
    .await;
    assert_eq!(status, 200);
}

async fn add(core: &Core, fake: &Fake, track: u64, guest: &str, ip: IpAddr) -> (u16, Value) {
    call(
        core,
        fake,
        "/api/add",
        json!({ "trackId": track, "guest": guest }),
        ip,
    )
    .await
}

#[tokio::test]
async fn a_track_already_waiting_is_refused() {
    let (core, fake) = (core(JamRules::default(), false), Fake::default());
    search(&core, &fake).await;
    assert_eq!(add(&core, &fake, 1, LEA, IP).await.0, 200);
    let (status, body) = add(&core, &fake, 1, MAX, OTHER_IP).await;
    assert_eq!((status, body["error"].as_str()), (409, Some("duplicate")));
    assert_eq!(*fake.added.lock().unwrap(), vec![1]);
}

#[tokio::test]
async fn each_guest_has_a_limit_of_waiting_tracks() {
    let rules = JamRules {
        max_pending: Some(1),
        ..JamRules::default()
    };
    let (core, fake) = (core(rules, false), Fake::default());
    search(&core, &fake).await;
    assert_eq!(add(&core, &fake, 1, LEA, IP).await.0, 200);
    let (status, body) = add(&core, &fake, 2, LEA, IP).await;
    assert_eq!((status, body["error"].as_str()), (429, Some("full")));
    // Un autre invité, même adresse (données mobiles) : sa propre limite.
    assert_eq!(add(&core, &fake, 2, MAX, IP).await.0, 200);

    // Le titre de Léa a été lu : elle peut en ajouter un autre.
    fake.added.lock().unwrap().retain(|id| *id != 1);
    assert_eq!(add(&core, &fake, 1, LEA, IP).await.0, 200);

    // Un album est coupé à la place restante.
    let core = self::core(
        JamRules {
            max_pending: Some(3),
            ..JamRules::default()
        },
        false,
    );
    let fake = Fake::default();
    search(&core, &fake).await;
    let body = json!({ "albumId": 40, "guest": LEA });
    let (status, body) = call(&core, &fake, "/api/add-album", body, IP).await;
    assert_eq!((status, body["count"].as_u64()), (200, Some(3)));
    assert_eq!(*fake.added.lock().unwrap(), vec![100, 101, 102]);
}

#[tokio::test]
async fn the_jam_share_of_the_queue_is_capped() {
    let guests = Arc::new(std::sync::Mutex::new(crate::jam::guests::Guests::default()));
    let core = Core::with_rules(CODE.into(), Arc::clone(&guests), JamRules::default(), false);
    let fake = Fake::default();
    // D'autres invités ont déjà mis 99 titres en attente.
    {
        let mut guests = guests.lock().unwrap();
        let batch = guests.new_batch();
        for id in 1000..1099 {
            guests.note(id, batch, None, Some(MAX.into()));
        }
    }
    fake.added.lock().unwrap().extend(1000..1099);
    search(&core, &fake).await;

    // L'album est coupé à la dernière place, puis la file de la Jam est pleine (#256).
    let body = json!({ "albumId": 40, "guest": LEA });
    let (status, body) = call(&core, &fake, "/api/add-album", body, IP).await;
    assert_eq!((status, body["count"].as_u64()), (200, Some(1)));
    let (status, body) = add(&core, &fake, 1, LEA, IP).await;
    assert_eq!((status, body["error"].as_str()), (429, Some("queue")));

    // Un titre de la Jam est lu : la place revient.
    fake.added.lock().unwrap().retain(|id| *id != 1000);
    assert_eq!(add(&core, &fake, 1, LEA, IP).await.0, 200);
}

#[tokio::test]
async fn adds_are_counted_per_guest_behind_a_shared_address() {
    let (core, fake) = (core(JamRules::default(), false), Fake::default());
    search(&core, &fake).await;
    // Léa épuise ses ajouts de la minute (les doublons comptent aussi).
    for _ in 0..crate::jam::guard::ADDS_PER_MINUTE {
        assert_ne!(add(&core, &fake, 1, LEA, IP).await.0, 429);
    }
    let (status, body) = add(&core, &fake, 2, LEA, IP).await;
    assert_eq!((status, body["error"].as_str()), (429, Some("slow")));
    // Max, derrière la même adresse (données mobiles), n'est pas freiné par elle (#256).
    assert_eq!(add(&core, &fake, 2, MAX, IP).await.0, 200);
}

#[tokio::test]
async fn the_host_can_refuse_albums_and_suspend_adds() {
    let rules = JamRules {
        albums: false,
        ..JamRules::default()
    };
    let (core, fake) = (core(rules, false), Fake::default());
    search(&core, &fake).await;
    let album = json!({ "albumId": 40, "guest": LEA });
    let (status, body) = call(&core, &fake, "/api/add-album", album.clone(), IP).await;
    assert_eq!((status, body["error"].as_str()), (403, Some("albums")));

    core.set_adds_open(false);
    let (status, body) = add(&core, &fake, 1, LEA, IP).await;
    assert_eq!((status, body["error"].as_str()), (403, Some("closed")));
    let state: Value = serde_json::from_str(&core.state_json(&fake).await.unwrap()).unwrap();
    assert_eq!(
        (state["adds"].clone(), state["albums"].clone()),
        (json!(false), json!(false))
    );
    assert!(state.get("limit").is_none());

    core.set_adds_open(true);
    core.set_rules(JamRules::default());
    assert_eq!(add(&core, &fake, 1, LEA, IP).await.0, 200);
    assert_eq!(call(&core, &fake, "/api/add-album", album, IP).await.0, 200);
}

#[tokio::test]
async fn a_banned_guest_loses_their_tracks_and_access() {
    let (core, fake) = (core(JamRules::default(), false), Fake::default());
    search(&core, &fake).await;
    let named = json!({ "trackId": 1, "guest": LEA, "name": "Léa" });
    assert_eq!(call(&core, &fake, "/api/add", named, IP).await.0, 200);
    assert_eq!(add(&core, &fake, 2, MAX, IP).await.0, 200);

    let members = core.members();
    assert_eq!(members.len(), 2);
    assert_eq!(members[0].name.as_deref(), Some("Léa"));
    assert!(core.set_banned(members[0].id, true, &fake).await);
    assert_eq!(*fake.added.lock().unwrap(), vec![2]);

    // Toutes ses requêtes sont refusées, celles de l'autre invité de la même adresse non.
    let (status, body) = call(&core, &fake, "/api/join", json!({ "guest": LEA }), IP).await;
    assert_eq!((status, body["error"].as_str()), (403, Some("banned")));
    let search = json!({ "query": "x", "guest": LEA });
    assert_eq!(call(&core, &fake, "/api/search", search, IP).await.0, 403);
    assert_eq!(add(&core, &fake, 1, MAX, IP).await.0, 200);

    // Réadmise.
    assert!(core.set_banned(members[0].id, false, &fake).await);
    assert!(!core.members()[0].banned);
    assert_eq!(
        call(&core, &fake, "/api/join", json!({ "guest": LEA }), IP)
            .await
            .0,
        200
    );
    assert!(!core.set_banned(999, true, &fake).await);
}

#[tokio::test]
async fn on_the_local_network_a_ban_blocks_the_device() {
    let (core, fake) = (core(JamRules::default(), true), Fake::default());
    search(&core, &fake).await;
    assert_eq!(add(&core, &fake, 1, LEA, IP).await.0, 200);
    core.set_banned(core.members()[0].id, true, &fake).await;
    // Nouveau jeton (navigation privée), même appareil : refusé.
    let (status, _) = call(&core, &fake, "/api/join", json!({ "guest": MAX }), IP).await;
    assert_eq!(status, 403);
    assert_eq!(
        call(&core, &fake, "/api/join", json!({}), OTHER_IP).await.0,
        200
    );
}
