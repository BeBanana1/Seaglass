//! Télécommande servie pour de vrai (`serve_site`) avec un faux lecteur.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

use super::*;
use crate::jam::guard::{COMMANDS_PER_MINUTE, MAX_FAILURES};
use crate::jam::remote::RemoteSite;
use crate::jam::server::serve_site;
use crate::jam::testing::{album, track};
use crate::player::volume::Volume as PlayerVolume;

const CODE: &str = "AB23CDEF45GH";
const PLAYLIST: &str = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";

#[derive(Default)]
struct FakeRemote {
    commands: Mutex<Vec<Command>>,
    /// Transferts reçus (`give`).
    given: Mutex<Vec<Handoff>>,
}

impl Remote for FakeRemote {
    async fn state(&self) -> Result<(PlayerState, QueueView), String> {
        let mut player = PlayerState {
            track: Some(track(7)),
            time_pos: 12.5,
            duration: 200.0,
            volume: PlayerVolume::default(),
            has_next: true,
            has_previous: true,
            ..PlayerState::default()
        };
        // Les commandes reçues changent l'état, comme le vrai lecteur (aperçu de la page).
        for command in self.commands.lock().unwrap().iter() {
            match command {
                Command::SetPaused(p) => player.paused = *p,
                Command::Seek(s) => player.time_pos = *s,
                Command::Volume(l) => player.volume.level = *l,
                Command::Muted(m) => player.volume.muted = *m,
                Command::Shuffle(s) => player.shuffle = *s,
                Command::Repeat(r) => player.repeat = *r,
                _ => {}
            }
        }
        let queue = QueueView {
            manual: vec![track(1)],
            upcoming: vec![track(8), track(9)],
            ..QueueView::default()
        };
        Ok((player, queue))
    }

    async fn run(&self, command: Command) -> Result<(), String> {
        self.commands.lock().unwrap().push(command);
        Ok(())
    }

    async fn search(&self, _query: String) -> Result<SearchResults, String> {
        Ok(SearchResults {
            tracks: vec![track(1), track(2)],
            albums: vec![album(40)],
            artists: Vec::new(),
            playlists: vec![playlist()],
        })
    }

    async fn album(&self, id: u64) -> Result<(String, Vec<Track>), String> {
        Ok(if id == 40 {
            ("Album 40".into(), (100..103).map(track).collect())
        } else {
            ("Vide".into(), Vec::new())
        })
    }

    async fn playlist(&self, uuid: String) -> Result<(String, Vec<Track>), String> {
        assert_eq!(uuid, PLAYLIST);
        Ok(("Ma playlist".into(), vec![track(5), track(6)]))
    }

    async fn playlists(&self) -> Result<Vec<PlaylistSummary>, String> {
        Ok(vec![playlist()])
    }

    async fn take_handoff(&self) -> Result<Option<Handoff>, String> {
        // Comme le vrai lecteur : il se met en pause en donnant la lecture.
        self.commands.lock().unwrap().push(Command::SetPaused(true));
        Ok(Some(handoff::sample(200)))
    }

    async fn give_handoff(&self, handoff: Handoff) -> Result<bool, String> {
        self.given.lock().unwrap().push(handoff);
        Ok(true)
    }
}

fn playlist() -> PlaylistSummary {
    PlaylistSummary {
        uuid: PLAYLIST.into(),
        title: "Ma playlist".into(),
        description: None,
        image_url: None,
        number_of_tracks: Some(2),
        duration: None,
        editorial: false,
        mine: true,
    }
}

async fn start() -> (SocketAddr, Arc<RemoteSite<FakeRemote>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let site = Arc::new(RemoteSite {
        owner: Arc::new(Owner::new(CODE.into())),
        remote: FakeRemote::default(),
    });
    tokio::spawn(serve_site(listener, Arc::clone(&site)));
    (addr, site)
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

async fn post(addr: SocketAddr, path: &str, body: &str) -> (u16, String) {
    call(addr, "POST", path, CODE, body).await
}

fn last(fake: &FakeRemote) -> Option<Command> {
    fake.commands.lock().unwrap().last().cloned()
}

#[tokio::test]
async fn serves_its_own_page_without_code() {
    let (addr, _site) = start().await;
    let (status, body) = call(addr, "GET", "/", "", "").await;
    assert_eq!(status, 200);
    assert!(body.contains("remote.js"));
    for file in ["/remote.js", "/remote.css", "/jam.css"] {
        assert_eq!(call(addr, "GET", file, "", "").await.0, 200, "{file}");
    }
    // La page des invités n'est pas servie ici.
    assert_eq!(call(addr, "GET", "/jam.js", "", "").await.0, 404);
}

#[tokio::test]
async fn only_the_owner_code_opens_the_remote() {
    let (addr, site) = start().await;
    let fake = &site.remote;
    // Un code de Jam (6 caractères) ou un code voisin n'ouvre rien.
    assert_eq!(call(addr, "POST", "/api/next", "AB23CD", "").await.0, 401);
    assert_eq!(
        call(addr, "POST", "/api/next", "AB23CDEF45GJ", "").await.0,
        401
    );
    assert!(fake.commands.lock().unwrap().is_empty());
    // Casse et tirets ignorés, comme pour la Jam.
    assert_eq!(
        call(addr, "POST", "/api/join", "ab23-cdef-45gh", "")
            .await
            .0,
        200
    );
    for _ in 2..MAX_FAILURES {
        assert_eq!(call(addr, "POST", "/api/join", "ZZZZ", "").await.0, 401);
    }
    assert_eq!(call(addr, "POST", "/api/join", CODE, "").await.0, 429);
    let events = format!("/api/events?code={CODE}");
    assert_eq!(call(addr, "GET", &events, "", "").await.0, 429);
}

#[tokio::test]
async fn a_new_code_locks_out_the_old_one() {
    let (addr, site) = start().await;
    site.owner.set_code("ZZ23CDEF45GH".into());
    assert_eq!(post(addr, "/api/join", "").await.0, 401);
    assert_eq!(
        call(addr, "POST", "/api/join", "ZZ23CDEF45GH", "").await.0,
        200
    );
}

#[tokio::test]
async fn transport_commands_reach_the_player() {
    let (addr, site) = start().await;
    let fake = &site.remote;
    let cases: &[(&str, &str, Command)] = &[
        ("/api/pause", r#"{"paused":true}"#, Command::SetPaused(true)),
        ("/api/next", "", Command::Next),
        ("/api/previous", "", Command::Previous),
        ("/api/seek", r#"{"seconds":42.5}"#, Command::Seek(42.5)),
        ("/api/volume", r#"{"level":35}"#, Command::Volume(35)),
        ("/api/mute", r#"{"muted":true}"#, Command::Muted(true)),
        (
            "/api/shuffle",
            r#"{"enabled":true}"#,
            Command::Shuffle(true),
        ),
        (
            "/api/repeat",
            r#"{"mode":"one"}"#,
            Command::Repeat(Repeat::One),
        ),
        (
            "/api/jump",
            r#"{"part":"upcoming","index":1,"trackId":9}"#,
            Command::Jump {
                part: QueuePart::Upcoming,
                index: 1,
                id: TrackId(9),
            },
        ),
        (
            "/api/remove",
            r#"{"trackId":1}"#,
            Command::Remove(TrackId(1)),
        ),
        (
            "/api/play",
            r#"{"trackId":2}"#,
            Command::Play {
                ids: vec![TrackId(2)],
                shuffle: false,
                source: None,
            },
        ),
        (
            "/api/enqueue",
            r#"{"trackId":2,"next":true}"#,
            Command::Enqueue {
                ids: vec![TrackId(2)],
                next: true,
            },
        ),
    ];
    for (path, body, expected) in cases {
        assert_eq!(post(addr, path, body).await, (200, r#"{"ok":true}"#.into()));
        assert_eq!(last(fake).as_ref(), Some(expected), "{path}");
    }
}

#[tokio::test]
async fn bad_values_are_refused() {
    let (addr, site) = start().await;
    let fake = &site.remote;
    for (path, body) in [
        ("/api/seek", r#"{"seconds":-1}"#),
        ("/api/volume", r#"{"level":101}"#),
        ("/api/volume", r#"{"level":-3}"#),
        ("/api/repeat", r#"{"mode":"twice"}"#),
        ("/api/pause", "{}"),
        ("/api/play-playlist", r#"{"uuid":"../users/1"}"#),
    ] {
        assert_eq!(post(addr, path, body).await.0, 400, "{path} {body}");
    }
    assert_eq!(post(addr, "/api/like", "{}").await.0, 404);
    assert!(fake.commands.lock().unwrap().is_empty());
}

#[tokio::test]
async fn albums_and_playlists_play_from_the_start() {
    let (addr, site) = start().await;
    let fake = &site.remote;
    let (status, _) = post(addr, "/api/play-album", r#"{"albumId":40,"shuffle":true}"#).await;
    assert_eq!(status, 200);
    assert_eq!(
        last(fake),
        Some(Command::Play {
            ids: vec![TrackId(100), TrackId(101), TrackId(102)],
            shuffle: true,
            source: Some(PlaySource::Album {
                id: 40,
                title: "Album 40".into()
            }),
        })
    );
    let body = format!(r#"{{"uuid":"{PLAYLIST}"}}"#);
    assert_eq!(post(addr, "/api/play-playlist", &body).await.0, 200);
    assert_eq!(
        last(fake),
        Some(Command::Play {
            ids: vec![TrackId(5), TrackId(6)],
            shuffle: false,
            source: Some(PlaySource::Playlist {
                uuid: PLAYLIST.into(),
                title: "Ma playlist".into()
            }),
        })
    );
    let (status, body) = post(addr, "/api/play-album", r#"{"albumId":41}"#).await;
    assert_eq!((status, body.as_str()), (400, r#"{"error":"empty"}"#));
}

#[tokio::test]
async fn search_and_playlists_are_read_only() {
    let (addr, site) = start().await;
    let fake = &site.remote;
    let (status, body) = post(addr, "/api/search", r#"{"query":" x "}"#).await;
    assert_eq!(status, 200);
    let hits: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(hits["tracks"][1]["title"], "Titre 2 (Live)");
    assert_eq!(hits["albums"][0]["id"], 40);
    assert_eq!(hits["playlists"][0]["uuid"], PLAYLIST);
    let (_, body) = post(addr, "/api/playlists", "").await;
    let list: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(list[0]["title"], "Ma playlist");
    assert!(fake.commands.lock().unwrap().is_empty());
}

#[tokio::test]
async fn commands_are_rate_limited() {
    let (addr, _site) = start().await;
    for _ in 0..COMMANDS_PER_MINUTE {
        assert_eq!(post(addr, "/api/next", "").await.0, 200);
    }
    assert_eq!(post(addr, "/api/next", "").await.0, 429);
    // L'état et la recherche ont leurs propres compteurs.
    assert_eq!(post(addr, "/api/state", "").await.0, 200);
    assert_eq!(post(addr, "/api/search", r#"{"query":"x"}"#).await.0, 200);
}

#[tokio::test]
async fn events_push_the_player_state() {
    let (addr, _site) = start().await;
    let mut s = TcpStream::connect(addr).await.unwrap();
    let req = format!("GET /api/events?code={CODE} HTTP/1.1\r\nHost: x\r\n\r\n");
    s.write_all(req.as_bytes()).await.unwrap();
    let mut lines = BufReader::new(s).lines();
    let mut next_data = async || loop {
        let line = tokio::time::timeout(Duration::from_secs(5), lines.next_line())
            .await
            .expect("aucun événement")
            .unwrap()
            .unwrap();
        if let Some(data) = line.strip_prefix("data: ") {
            return serde_json::from_str::<serde_json::Value>(data).unwrap();
        }
    };
    let first = next_data().await;
    assert_eq!(first["now"]["id"], 7);
    assert_eq!(first["position"], 12);
    assert_eq!(first["paused"], false);
    assert_eq!(first["manual"][0]["id"], 1);
    assert_eq!(first["upcoming"][1]["id"], 9);

    assert_eq!(post(addr, "/api/pause", r#"{"paused":true}"#).await.0, 200);
    assert_eq!(next_data().await["paused"], true);
}

/// Page servie avec ce faux lecteur, pour la regarder dans un navigateur (30 min) :
/// `cargo test remote_page_preview -- --ignored --nocapture`, puis
/// `http://127.0.0.1:47899/#AB23CDEF45GH`.
#[tokio::test]
#[ignore]
async fn remote_page_preview() {
    let listener = TcpListener::bind("127.0.0.1:47899").await.unwrap();
    let site = Arc::new(RemoteSite {
        owner: Arc::new(Owner::new(CODE.into())),
        remote: FakeRemote::default(),
    });
    println!("http://127.0.0.1:47899/#{CODE}");
    let _ = tokio::time::timeout(Duration::from_secs(30 * 60), serve_site(listener, site)).await;
}

mod handoff;
