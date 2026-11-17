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

/// Volume des invités (#449) : fermé par défaut, plafonné, refusé quand l'hôte a coupé le son.
#[tokio::test]
async fn guests_set_the_volume_under_the_host_cap() {
    let (core, fake) = (core(JamRules::default(), false), Fake::default());
    let volume = |level: Value| json!({ "guest": LEA, "level": level });
    let (status, body) = call(&core, &fake, "/api/volume", volume(json!(40)), IP).await;
    assert_eq!((status, body["error"].as_str()), (403, Some("volume")));
    let state: Value = serde_json::from_str(&core.state_json(&fake).await.unwrap()).unwrap();
    assert!(state.get("volume").is_none(), "volume caché sans la règle");

    core.set_rules(JamRules {
        guest_volume: true,
        guest_volume_max: 70,
        ..JamRules::default()
    });
    // L'hôte est à 100 : un invité qui monte au-delà du plafond est ramené à 70.
    let (status, body) = call(&core, &fake, "/api/volume", volume(json!(95)), IP).await;
    assert_eq!((status, body["level"].as_u64()), (200, Some(70)));
    assert_eq!(fake.volume.lock().unwrap().level, 70);
    assert_eq!(
        call(&core, &fake, "/api/volume", volume(json!(30)), IP)
            .await
            .0,
        200
    );
    let state: Value = serde_json::from_str(&core.state_json(&fake).await.unwrap()).unwrap();
    assert_eq!(
        state["volume"],
        json!({ "level": 30, "max": 70, "muted": false })
    );

    for bad in [json!(101), json!(-3), json!("x")] {
        assert_eq!(
            call(&core, &fake, "/api/volume", volume(bad), IP).await.0,
            400
        );
    }

    // Son coupé par l'hôte : un invité ne le rétablit pas.
    fake.volume.lock().unwrap().set_muted(true);
    let (status, body) = call(&core, &fake, "/api/volume", volume(json!(50)), IP).await;
    assert_eq!((status, body["error"].as_str()), (409, Some("muted")));
    assert!(fake.volume.lock().unwrap().muted);
    fake.volume.lock().unwrap().set_muted(false);

    // Compté à part de la pause, par invité.
    for _ in 3..crate::jam::guard::VOLUMES_PER_MINUTE {
        assert_eq!(
            call(&core, &fake, "/api/volume", volume(json!(20)), IP)
                .await
                .0,
            200
        );
    }
    let (status, body) = call(&core, &fake, "/api/volume", volume(json!(20)), IP).await;
    assert_eq!((status, body["error"].as_str()), (429, Some("slow")));
    let pause = json!({ "guest": LEA, "paused": true });
    assert_eq!(call(&core, &fake, "/api/pause", pause, IP).await.0, 200);
}

/// Vote pour passer (#433) : fermé par défaut, un vote par invité, seuil de l'hôte, titre
/// changé entre-temps, blind test.
#[tokio::test]
async fn guests_vote_to_skip_the_current_track() {
    const ZOE: &str = "cccccccccccccccccccccccccccccccc";
    let (core, fake) = (core(JamRules::default(), false), Fake::default());
    let vote = |guest: &str, track: u64| json!({ "guest": guest, "trackId": track });
    let (status, body) = call(&core, &fake, "/api/skip", vote(LEA, 7), IP).await;
    assert_eq!((status, body["error"].as_str()), (403, Some("skip")));
    let state: Value = serde_json::from_str(&core.state_json(&fake).await.unwrap()).unwrap();
    assert!(state.get("skip").is_none(), "vote caché sans la règle");

    core.set_rules(JamRules {
        skip_vote: true,
        skip_votes: 3,
        ..JamRules::default()
    });
    let state: Value = serde_json::from_str(&core.state_json(&fake).await.unwrap()).unwrap();
    assert_eq!(state["skip"], json!({ "votes": 0, "needed": 3 }));

    let (status, body) = call(&core, &fake, "/api/skip", vote(LEA, 7), IP).await;
    assert_eq!((status, body["votes"].as_u64()), (200, Some(1)));
    // Même invité : compté une fois. Par le relais, l'adresse partagée ne compte pas.
    assert_eq!(
        call(&core, &fake, "/api/skip", vote(LEA, 7), IP).await.1["votes"],
        1
    );
    assert_eq!(
        call(&core, &fake, "/api/skip", vote(MAX, 7), IP).await.1["votes"],
        2
    );
    // Vote parti pour le titre d'avant : refusé, rien n'est compté.
    let (status, body) = call(&core, &fake, "/api/skip", vote(ZOE, 6), IP).await;
    assert_eq!((status, body["error"].as_str()), (409, Some("changed")));
    assert_eq!(*fake.skips.lock().unwrap(), 0);

    let (status, body) = call(&core, &fake, "/api/skip", vote(ZOE, 7), IP).await;
    assert_eq!(status, 200);
    assert_eq!(body["skipped"], true);
    assert_eq!(*fake.skips.lock().unwrap(), 1);

    // Nouveau titre : les votes repartent de zéro.
    *fake.now.lock().unwrap() = Some(Some(8));
    let state: Value = serde_json::from_str(&core.state_json(&fake).await.unwrap()).unwrap();
    assert_eq!(state["skip"]["votes"], 0);

    // Blind test ouvert aux invités : pas de vote.
    *fake.blind.lock().unwrap() = Some(crate::jam::testing::blind_game(true));
    let (status, body) = call(&core, &fake, "/api/skip", vote(LEA, 8), IP).await;
    assert_eq!((status, body["error"].as_str()), (409, Some("game")));
    let state: Value = serde_json::from_str(&core.state_json(&fake).await.unwrap()).unwrap();
    assert!(state.get("skip").is_none());
}

/// Réseau local : un appareil ne vote qu'une fois, même avec un autre jeton.
#[tokio::test]
async fn on_the_local_network_a_device_votes_once() {
    let rules = JamRules {
        skip_vote: true,
        skip_votes: 2,
        ..JamRules::default()
    };
    let (core, fake) = (core(rules, true), Fake::default());
    let vote = |guest: &str| json!({ "guest": guest, "trackId": 7 });
    assert_eq!(
        call(&core, &fake, "/api/skip", vote(LEA), IP).await.1["votes"],
        1
    );
    assert_eq!(
        call(&core, &fake, "/api/skip", vote(MAX), IP).await.1["votes"],
        1
    );
    assert_eq!(*fake.skips.lock().unwrap(), 0);
    let (_, body) = call(&core, &fake, "/api/skip", vote(MAX), OTHER_IP).await;
    assert_eq!(body["skipped"], true);
    assert_eq!(*fake.skips.lock().unwrap(), 1);
}

/// Paroles du titre en cours (#448) : une requête à TIDAL pour plusieurs invités, rien sans
/// titre, refus pendant un blind test, position dans l'état.
#[tokio::test]
async fn guests_read_the_lyrics_of_the_current_track() {
    let (core, fake) = (core(JamRules::default(), false), Fake::default());
    let state =
        || async { serde_json::from_str::<Value>(&core.state_json(&fake).await.unwrap()).unwrap() };
    // Avant toute demande : on ne sait pas, le bouton reste affiché.
    let before = state().await;
    assert!(before.get("lyrics").is_none());
    assert!(before["startedAt"].as_i64().is_some(), "{before}");

    let ask =
        |guest: &'static str, ip| call(&core, &fake, "/api/lyrics", json!({ "guest": guest }), ip);
    let ((s1, b1), (s2, b2)) = tokio::join!(ask(LEA, IP), ask(MAX, OTHER_IP));
    assert_eq!((s1, s2), (200, 200));
    assert_eq!(b1["trackId"], 7);
    assert_eq!(b1["lyrics"]["text"]["kind"], "synced");
    assert_eq!(b1["lyrics"]["provider"], "MUSIXMATCH");
    assert_eq!(b1["lyrics"], b2["lyrics"]);
    assert!(b1["now"].as_i64().is_some());
    assert_eq!(
        *fake.lyrics_calls.lock().unwrap(),
        vec![7],
        "une seule requête"
    );
    assert_eq!(state().await["lyrics"], true);

    // Autre titre sans paroles : réponse vide, retenue aussi.
    *fake.now.lock().unwrap() = Some(Some(8));
    let (status, body) = ask(LEA, IP).await;
    assert_eq!((status, body["trackId"].as_u64()), (200, Some(8)));
    assert!(body["lyrics"].is_null());
    ask(MAX, OTHER_IP).await;
    assert_eq!(*fake.lyrics_calls.lock().unwrap(), vec![7, 8]);
    assert_eq!(state().await["lyrics"], false);

    // En pause : la position, pas de point de départ.
    *fake.paused.lock().unwrap() = Some(true);
    let paused = state().await;
    assert_eq!(paused["position"], 42_000);
    assert!(paused.get("startedAt").is_none());

    // Rien en lecture.
    *fake.now.lock().unwrap() = Some(None);
    let (status, body) = ask(LEA, IP).await;
    assert_eq!((status, body["trackId"].clone()), (200, Value::Null));

    // Blind test : les paroles donneraient la réponse.
    *fake.now.lock().unwrap() = None;
    *fake.blind.lock().unwrap() = Some(crate::jam::testing::blind_game(false));
    let (status, body) = ask(LEA, IP).await;
    assert_eq!((status, body["error"].as_str()), (409, Some("game")));
    let during = state().await;
    assert!(during.get("lyrics").is_none() && during.get("position").is_none());
}

/// Blind test à plusieurs (#441) : réponses, refus, partie dans l'état, ajouts suspendus.
#[tokio::test]
async fn guests_answer_the_blind_test() {
    let core = core(JamRules::default(), false);
    let fake = Fake::default();
    let answer = |guest: &str, round: usize, extra: Value| {
        let mut body = json!({ "guest": guest, "round": round, "name": "Léa" });
        if let (Some(b), Some(e)) = (body.as_object_mut(), extra.as_object()) {
            b.extend(e.clone());
        }
        body
    };
    // Sans partie : refusé.
    let (status, body) = call(
        &core,
        &fake,
        "/api/blind-answer",
        answer(LEA, 2, json!({})),
        IP,
    )
    .await;
    assert_eq!((status, body["error"].as_str()), (409, Some("round")));

    // Une partie en solo ne regarde pas les invités : ni dans l'état, ni pour répondre.
    *fake.blind.lock().unwrap() = Some(crate::jam::testing::blind_game(false));
    let state = core.state_json(&fake).await.unwrap();
    assert!(!state.contains("\"blind\""), "{state}");
    assert_eq!(
        call(
            &core,
            &fake,
            "/api/blind-answer",
            answer(LEA, 2, json!({})),
            IP
        )
        .await
        .0,
        409
    );

    *fake.blind.lock().unwrap() = Some(crate::jam::testing::blind_game(true));
    let state: Value = serde_json::from_str(&core.state_json(&fake).await.unwrap()).unwrap();
    assert_eq!(state["blind"]["round"], 2);
    assert_eq!(state["blind"]["last"], Value::Null);
    assert_eq!(state["adds"], false, "ajouts suspendus pendant la partie");

    // Manche périmée, puis bonne manche, puis doublon.
    assert_eq!(
        call(
            &core,
            &fake,
            "/api/blind-answer",
            answer(LEA, 1, json!({ "text": "x" })),
            IP
        )
        .await
        .0,
        409
    );
    let (status, body) = call(
        &core,
        &fake,
        "/api/blind-answer",
        answer(LEA, 2, json!({ "text": "  One More Time  " })),
        IP,
    )
    .await;
    assert_eq!((status, body["player"].as_u64()), (200, Some(1)));
    let (status, body) = call(
        &core,
        &fake,
        "/api/blind-answer",
        answer(LEA, 2, json!({ "choice": 1 })),
        IP,
    )
    .await;
    assert_eq!((status, body["error"].as_str()), (409, Some("answered")));
    call(
        &core,
        &fake,
        "/api/blind-answer",
        json!({ "guest": MAX, "round": 2 }),
        OTHER_IP,
    )
    .await;
    {
        let answers = fake.answers.lock().unwrap();
        assert_eq!(answers[0].1.as_deref(), Some("Léa"));
        assert_eq!(
            answers[0].3,
            crate::player::blind::Answer::Text {
                text: "One More Time".into()
            }
        );
        assert_eq!(
            answers[1].3,
            crate::player::blind::Answer::Pass,
            "réponse vide = passer"
        );
    }
    // Les joueurs apparaissent chez l'hôte, qui peut les exclure.
    assert_eq!(core.members().len(), 2);

    // Ajouts refusés tant que la partie dure.
    search(&core, &fake).await;
    let (status, body) = add(&core, &fake, 1, LEA, IP).await;
    assert_eq!((status, body["error"].as_str()), (403, Some("game")));
}
