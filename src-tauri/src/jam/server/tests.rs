use super::*;
use crate::jam::api::{MAX_ALBUM_TRACKS, MAX_QUERY_CHARS, MAX_QUEUE};
use crate::jam::testing::Fake;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

const CODE: &str = "AB23CD";

async fn start() -> (SocketAddr, Arc<Fake>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let fake = Arc::new(Fake::default());
    let core = Arc::new(Core::new(CODE.into(), Arc::default()));
    tokio::spawn(serve(listener, core, Arc::clone(&fake)));
    (addr, fake)
}

async fn call(addr: SocketAddr, method: &str, path: &str, code: &str, body: &str) -> (u16, String) {
    let mut s = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: x\r\nX-Jam-Code: {code}\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).await.unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    let status = out[9..12].parse().unwrap();
    let body = out.split_once("\r\n\r\n").unwrap().1.to_owned();
    (status, body)
}

async fn search(addr: SocketAddr) {
    let (status, _) = call(addr, "POST", "/api/search", CODE, r#"{"query":"x"}"#).await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn serves_the_page_without_code() {
    let (addr, _) = start().await;
    let (status, body) = call(addr, "GET", "/", "", "").await;
    assert_eq!(status, 200);
    assert!(body.contains("jam.js"));
    assert_eq!(call(addr, "GET", "/jam.js", "", "").await.0, 200);
    assert_eq!(call(addr, "GET", "/secret", "", "").await.0, 404);
}

/// Blind test with several players (#441): the single-segment route also goes through the
/// relay.
#[tokio::test]
async fn guests_answer_the_blind_test_over_http() {
    let (addr, fake) = start().await;
    *fake.blind.lock().unwrap() = Some(crate::jam::testing::blind_game(true));
    let body = r#"{"guest":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","round":2,"text":"x"}"#;
    let (status, reply) = call(addr, "POST", "/api/blind-answer", CODE, body).await;
    assert_eq!((status, reply.as_str()), (200, r#"{"player":1}"#));
    assert_eq!(
        call(addr, "POST", "/api/blind-answer", "ZZZZZZ", body)
            .await
            .0,
        401
    );
}

/// Preview of the guest page during a blind test (#441):
/// `cargo test jam_blind_preview -- --ignored --nocapture`, then the address shown
/// (10 min). The round is revealed 3 s after the first answer, then the next one starts
/// 8 s later.
#[tokio::test]
#[ignore]
async fn jam_blind_preview() {
    use crate::jam::testing::{blind_game, track};
    use crate::player::blind::{Phase, PlayerResult, PlayerView, RoundResult};
    let listener = TcpListener::bind("127.0.0.1:47898").await.unwrap();
    let fake = Arc::new(Fake::default());
    let mut game = blind_game(true);
    game.players.push(PlayerView {
        id: 2,
        name: Some("Max".into()),
        host: false,
        score: 4,
        streak: 2,
        best_streak: 2,
        answered: true,
    });
    *fake.blind.lock().unwrap() = Some(game);
    let core = Arc::new(Core::new(CODE.into(), Arc::default()));
    tokio::spawn(serve(listener, core, Arc::clone(&fake)));
    println!("http://127.0.0.1:47898/#{CODE}");
    let host = Arc::clone(&fake);
    let script = async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let answered = host.answers.lock().unwrap().len();
            if answered == 0 {
                continue;
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
            let results = |id: u32, host: bool, points: u32| PlayerResult {
                id,
                name: (!host).then(|| if id == 1 { "Léa".into() } else { "Max".into() }),
                host,
                answered: true,
                title: points >= 2,
                artist: points % 2 == 1,
                points,
                fastest: points == 4,
            };
            {
                let mut blind = host.blind.lock().unwrap();
                let Some(game) = blind.as_mut() else { return };
                game.phase = Phase::Revealed;
                game.results.push(RoundResult {
                    track: track(5),
                    title: true,
                    artist: true,
                    points: 4,
                    answer: None,
                    chosen: None,
                    correct: None,
                    players: vec![
                        results(0, true, 4),
                        results(1, false, 1),
                        results(2, false, 0),
                    ],
                });
                game.players = vec![
                    PlayerView {
                        id: 0,
                        name: None,
                        host: true,
                        score: 7,
                        streak: 2,
                        best_streak: 2,
                        answered: false,
                    },
                    PlayerView {
                        id: 2,
                        name: Some("Max".into()),
                        host: false,
                        score: 4,
                        streak: 0,
                        best_streak: 2,
                        answered: false,
                    },
                    PlayerView {
                        id: 1,
                        name: Some("Léa".into()),
                        host: false,
                        score: 1,
                        streak: 1,
                        best_streak: 1,
                        answered: false,
                    },
                ];
            }
            tokio::time::sleep(Duration::from_secs(8)).await;
            let mut blind = host.blind.lock().unwrap();
            let Some(game) = blind.as_mut() else { return };
            game.phase = Phase::Guess;
            game.round += 1;
            game.reveal_at = Some(crate::jam::testing::now_ms() + 30_000);
            host.answers.lock().unwrap().clear();
        }
    };
    let _ = tokio::time::timeout(Duration::from_secs(600), script).await;
}

#[tokio::test]
async fn api_requires_the_code_and_blocks_guessing() {
    let (addr, _) = start().await;
    assert_eq!(call(addr, "POST", "/api/join", "ab2-3cd", "").await.0, 200);
    for _ in 0..crate::jam::guard::MAX_FAILURES {
        assert_eq!(call(addr, "POST", "/api/join", "ZZZZZZ", "").await.0, 401);
    }
    // Blocked: even the right code is refused, the stream included.
    assert_eq!(call(addr, "POST", "/api/join", CODE, "").await.0, 429);
    let events = format!("/api/events?code={CODE}");
    assert_eq!(call(addr, "GET", &events, "", "").await.0, 429);
}

#[tokio::test]
async fn only_searched_tracks_can_be_added() {
    let (addr, fake) = start().await;
    let add = r#"{"trackId":2}"#;
    let (status, body) = call(addr, "POST", "/api/add", CODE, add).await;
    assert_eq!((status, body.as_str()), (400, r#"{"error":"unknown"}"#));

    let (status, body) = call(addr, "POST", "/api/search", CODE, r#"{"query":" x "}"#).await;
    assert_eq!(status, 200);
    let results: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(results[1]["title"], "Titre 2 (Live)");
    assert_eq!(results[1]["artists"], "A, B");
    assert!(results[0].get("isrc").is_none());
    assert!(results[0].get("by").is_none());

    assert_eq!(call(addr, "POST", "/api/add", CODE, add).await.0, 200);
    assert_eq!(*fake.added.lock().unwrap(), vec![2]);
    assert_eq!(call(addr, "POST", "/api/add", CODE, "{").await.0, 400);
}

#[tokio::test]
async fn guest_names_reach_the_host_and_the_queue() {
    let (addr, fake) = start().await;
    search(addr).await;
    let add = r#"{"trackId":1,"name":"  Léa \n"}"#;
    assert_eq!(call(addr, "POST", "/api/add", CODE, add).await.0, 200);
    assert_eq!(*fake.named.lock().unwrap(), vec![(1, "Léa".to_owned())]);
    // Without a name: added, but anonymous.
    assert_eq!(
        call(addr, "POST", "/api/add", CODE, r#"{"trackId":2}"#)
            .await
            .0,
        200
    );

    let (_, body) = call(addr, "POST", "/api/state", CODE, "").await;
    let state: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(state["now"]["id"], 7);
    assert_eq!(state["queue"][0]["by"], "Léa");
    assert!(state["queue"][1].get("by").is_none());
    assert_eq!(state["queue"].as_array().unwrap().len(), MAX_QUEUE);
}

#[tokio::test]
async fn events_push_the_state_and_its_changes() {
    let (addr, _) = start().await;
    let mut s = TcpStream::connect(addr).await.unwrap();
    let req = format!("GET /api/events?code={CODE} HTTP/1.1\r\nHost: x\r\n\r\n");
    s.write_all(req.as_bytes()).await.unwrap();
    let mut lines = BufReader::new(s).lines();
    let mut next_data = async || loop {
        let line = tokio::time::timeout(Duration::from_secs(5), lines.next_line())
            .await
            .expect("no event")
            .unwrap()
            .unwrap();
        if let Some(data) = line.strip_prefix("data: ") {
            return serde_json::from_str::<serde_json::Value>(data).unwrap();
        }
    };
    let first = next_data().await;
    assert_eq!(first["now"]["id"], 7);
    assert_eq!(first["queue"][0]["id"], 8);

    search(addr).await;
    let add = r#"{"trackId":1,"name":"Max"}"#;
    assert_eq!(call(addr, "POST", "/api/add", CODE, add).await.0, 200);
    let second = next_data().await;
    assert_eq!(second["queue"][0]["id"], 1);
    assert_eq!(second["queue"][0]["by"], "Max");
}

#[tokio::test]
async fn guests_never_reach_the_owner_commands() {
    let (addr, _) = start().await;
    // Remote control commands (#336): unknown to the Jam server. The volume has its
    // guest route, closed by default (#449).
    assert_eq!(call(addr, "POST", "/api/volume", CODE, "{}").await.0, 403);
    for path in ["/api/next", "/api/seek", "/api/play-album"] {
        assert_eq!(call(addr, "POST", path, CODE, "{}").await.0, 404, "{path}");
    }
    assert_eq!(call(addr, "GET", "/remote.js", "", "").await.0, 404);
}

#[tokio::test]
async fn events_need_the_code() {
    let (addr, _) = start().await;
    assert_eq!(
        call(addr, "GET", "/api/events?code=ZZZZZZ", "", "").await.0,
        401
    );
}

#[tokio::test]
async fn guests_pause_only_when_the_host_allows_it() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let fake = Arc::new(Fake::default());
    let core = Arc::new(Core::new(CODE.into(), Arc::default()));
    tokio::spawn(serve(listener, Arc::clone(&core), Arc::clone(&fake)));

    let pause = r#"{"paused":true}"#;
    assert_eq!(call(addr, "POST", "/api/pause", CODE, pause).await.0, 200);
    assert_eq!(*fake.paused.lock().unwrap(), Some(true));
    let (_, body) = call(addr, "POST", "/api/state", CODE, "").await;
    let state: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        (state["paused"].clone(), state["controls"].clone()),
        (true.into(), true.into())
    );
    assert_eq!(call(addr, "POST", "/api/pause", CODE, "{}").await.0, 400);

    core.set_controls(false);
    let (status, body) = call(addr, "POST", "/api/pause", CODE, r#"{"paused":false}"#).await;
    assert_eq!((status, body.as_str()), (403, r#"{"error":"controls"}"#));
    assert_eq!(*fake.paused.lock().unwrap(), Some(true));
    let (_, body) = call(addr, "POST", "/api/state", CODE, "").await;
    let state: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(state["controls"], false);

    // Limit per address.
    core.set_controls(true);
    for _ in 1..crate::jam::guard::CONTROLS_PER_MINUTE {
        assert_eq!(call(addr, "POST", "/api/pause", CODE, pause).await.0, 200);
    }
    assert_eq!(call(addr, "POST", "/api/pause", CODE, pause).await.0, 429);
}

#[tokio::test]
async fn search_returns_albums_only_to_pages_that_ask() {
    let (addr, _) = start().await;
    let (_, body) = call(addr, "POST", "/api/search", CODE, r#"{"query":"x"}"#).await;
    let old: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(old.is_array());

    let query = r#"{"query":"x","albums":true}"#;
    let (_, body) = call(addr, "POST", "/api/search", CODE, query).await;
    let hits: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(hits["tracks"][0]["id"], 1);
    assert_eq!(hits["albums"][0]["id"], 40);
    assert_eq!(hits["albums"][0]["artists"], "A");
    assert_eq!(hits["albums"][0]["tracks"], 35);
    assert_eq!(hits["albums"][0]["year"], "2013");
}

#[tokio::test]
async fn albums_are_added_as_one_batch_that_only_its_owner_removes() {
    let (addr, fake) = start().await;
    let (lea, max) = ("a".repeat(32), "b".repeat(32));
    let album = format!(r#"{{"albumId":40,"name":"Léa","guest":"{lea}"}}"#);
    // Not offered by a search yet.
    assert_eq!(
        call(addr, "POST", "/api/add-album", CODE, &album).await.0,
        400
    );

    let query = r#"{"query":"x","albums":true}"#;
    assert_eq!(call(addr, "POST", "/api/search", CODE, query).await.0, 200);
    let (status, body) = call(addr, "POST", "/api/add-album", CODE, &album).await;
    assert_eq!(status, 200);
    let added: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(added["count"], MAX_ALBUM_TRACKS);
    let batch = added["batch"].as_u64().unwrap();
    assert_eq!(fake.added.lock().unwrap().len(), MAX_ALBUM_TRACKS);

    let (_, body) = call(addr, "POST", "/api/state", CODE, "").await;
    let state: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(state["queue"][0]["id"], 100);
    assert_eq!(state["queue"][0]["by"], "Léa");
    assert_eq!(state["queue"][0]["batch"], batch);

    // Another token removes nothing; without a valid token, the request is refused.
    let remove =
        |guest: &str, track: &str| format!(r#"{{"guest":"{guest}","batch":{batch}{track}}}"#);
    let (_, body) = call(addr, "POST", "/api/remove", CODE, &remove(&max, "")).await;
    assert_eq!(body, r#"{"ok":true,"removed":0}"#);
    assert_eq!(
        call(addr, "POST", "/api/remove", CODE, &remove("x", ""))
            .await
            .0,
        400
    );

    // One track, then the rest of the batch.
    let one = remove(&lea, r#","trackId":101"#);
    let (_, body) = call(addr, "POST", "/api/remove", CODE, &one).await;
    assert_eq!(body, r#"{"ok":true,"removed":1}"#);
    assert!(!fake.added.lock().unwrap().contains(&101));
    let (_, body) = call(addr, "POST", "/api/remove", CODE, &remove(&lea, "")).await;
    assert_eq!(
        body,
        format!(r#"{{"ok":true,"removed":{}}}"#, MAX_ALBUM_TRACKS - 1)
    );
    assert!(fake.added.lock().unwrap().is_empty());
}

#[tokio::test]
async fn pasted_links_are_resolved_instead_of_searched() {
    let (addr, _) = start().await;
    let query =
        r#"{"query":"https://open.spotify.com/album/4m2880jivSbbyEGAKfITCa","albums":true}"#;
    let (_, body) = call(addr, "POST", "/api/search", CODE, query).await;
    let hits: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(hits["link"], true);
    assert_eq!(hits["albums"][0]["id"], 41);
    // The link's album is offered like a searched one (the fake has no tracks for
    // it: "empty", not "unknown").
    let add = r#"{"albumId":41}"#;
    let (_, body) = call(addr, "POST", "/api/add-album", CODE, add).await;
    assert_eq!(body, r#"{"error":"empty"}"#);
    let (_, body) = call(
        addr,
        "POST",
        "/api/search",
        CODE,
        r#"{"query":"x","albums":true}"#,
    )
    .await;
    let hits: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(hits["link"], false);
}

#[tokio::test]
async fn long_queries_are_refused() {
    let (addr, _) = start().await;
    let body = format!(r#"{{"query":"{}"}}"#, "a".repeat(MAX_QUERY_CHARS + 1));
    assert_eq!(call(addr, "POST", "/api/search", CODE, &body).await.0, 400);
}
