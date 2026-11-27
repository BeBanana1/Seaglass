//! Playback handoffs (#414) served for real, with the fake player.

use rand::SeedableRng;
use rand::rngs::StdRng;

use super::*;
use crate::player::queue::Queue;

/// Handoff of a context of `len` tracks, on the second one, at 83 s.
pub(super) fn sample(len: u64) -> Handoff {
    let mut queue = Queue::default();
    queue.set_context(
        (1..=len).map(TrackId).collect(),
        1,
        None,
        &mut StdRng::seed_from_u64(5),
    );
    Handoff {
        queue,
        position: 83.0,
        tracks: vec![track(2)],
    }
}

async fn start_with(remote: bool, handoff: bool) -> (SocketAddr, Arc<RemoteSite<FakeRemote>>) {
    let (addr, site) = start().await;
    site.owner.set_features(remote, handoff);
    (addr, site)
}

#[tokio::test]
async fn transfers_are_refused_unless_accepted() {
    let (addr, site) = start().await;
    assert_eq!(post(addr, "/api/handoff/take", "").await.0, 403);
    assert_eq!(post(addr, "/api/handoff/give", "{}").await.0, 403);
    assert!(site.remote.commands.lock().unwrap().is_empty());
}

#[tokio::test]
async fn take_returns_the_queue_and_pauses() {
    let (addr, site) = start_with(false, true).await;
    let (status, body) = post(addr, "/api/handoff/take", "").await;
    assert_eq!(status, 200);
    let taken: Handoff = serde_json::from_str(&body).unwrap();
    assert_eq!(taken, sample(200));
    assert_eq!(last(&site.remote), Some(Command::SetPaused(true)));
}

#[tokio::test]
async fn give_accepts_a_large_queue_with_the_code() {
    let (addr, site) = start_with(false, true).await;
    // 2,000 tracks: far more than the 2 KiB of the other routes.
    let sent = sample(2_000);
    let body = serde_json::to_string(&sent).unwrap();
    assert!(body.len() > 8 * 1024);
    assert_eq!(post(addr, "/api/handoff/give", &body).await.0, 200);
    assert_eq!(site.remote.given.lock().unwrap().as_slice(), &[sent]);
}

#[tokio::test]
async fn a_large_body_without_the_code_is_never_read() {
    let (addr, site) = start_with(false, true).await;
    let body = serde_json::to_string(&sample(2_000)).unwrap();
    let mut s = TcpStream::connect(addr).await.unwrap();
    let head = format!(
        "POST /api/handoff/give HTTP/1.1\r\nHost: x\r\nX-Jam-Code: ZZZZZZZZZZZZ\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    s.write_all(head.as_bytes()).await.unwrap();
    let _ = s.write_all(body.as_bytes()).await;
    let mut out = String::new();
    let _ = s.read_to_string(&mut out).await;
    // Connection closed without a response: the body was not read.
    assert!(out.is_empty(), "{out}");
    assert!(site.remote.given.lock().unwrap().is_empty());
}

#[tokio::test]
async fn an_inconsistent_queue_is_refused() {
    let (addr, site) = start_with(false, true).await;
    let mut json: serde_json::Value = serde_json::to_value(sample(5)).unwrap();
    json["queue"]["order"] = serde_json::json!([0, 0, 1, 2, 3]);
    let (status, _) = post(addr, "/api/handoff/give", &json.to_string()).await;
    assert_eq!(status, 400);
    assert_eq!(post(addr, "/api/handoff/give", "pas du json").await.0, 400);
    assert!(site.remote.given.lock().unwrap().is_empty());
}

#[tokio::test]
async fn transfers_alone_serve_nothing_of_the_remote() {
    let (addr, site) = start_with(false, true).await;
    assert_eq!(call(addr, "GET", "/", "", "").await.0, 404);
    assert_eq!(call(addr, "GET", "/remote.js", "", "").await.0, 404);
    assert_eq!(post(addr, "/api/next", "").await.0, 404);
    assert_eq!(post(addr, "/api/state", "").await.0, 404);
    // The phone checks the link with `join`.
    assert_eq!(post(addr, "/api/join", "").await.0, 200);
    assert!(site.remote.commands.lock().unwrap().is_empty());
}

/// The phone's client (`crate::handoff`) against the real server.
#[tokio::test]
async fn the_phone_client_talks_to_the_server() {
    let (addr, site) = start_with(false, false).await;
    let link = crate::handoff::Link::parse(&format!("http://{addr}/#ab23-cdef-45gh")).unwrap();
    link.check().await.unwrap();
    // Handoffs refused as long as the PC does not accept them: clear message.
    let refused = link.take().await.unwrap_err();
    assert!(refused.contains("transfer"), "{refused}");

    site.owner.set_features(false, true);
    assert_eq!(link.take().await.unwrap(), sample(200));
    let sent = sample(3_000);
    link.give(&sent).await.unwrap();
    assert_eq!(site.remote.given.lock().unwrap().as_slice(), &[sent]);

    let wrong = crate::handoff::Link::parse(&format!("http://{addr}/#ZZ23CDEF45GH")).unwrap();
    assert!(wrong.check().await.is_err());
}
