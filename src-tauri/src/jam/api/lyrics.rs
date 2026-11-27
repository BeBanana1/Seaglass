//! Lyrics of the current track for guests (#448). Never an id as a parameter:
//! the Jam does not become a relay to TIDAL for any track. One request to
//! TIDAL per track, however many guests open the lyrics.

use std::net::IpAddr;
use std::sync::Arc;

use super::{BACKEND_TIMEOUT, Backend, Core, backend_error};
use crate::jam::clock::now_ms;
use crate::jam::guard::Action;
use crate::jam::http::Response;
use crate::tidal::models::Lyrics;

/// Lyrics of the last track requested; `None` = the track has none.
pub(super) type CachedLyrics = (u64, Option<Arc<Lyrics>>);

impl Core {
    /// `POST api/lyrics {guest}`: `{trackId, lyrics, now}`, `now` being the host's time
    /// (the page thus corrects its clock's offset to follow the lines). Refused during
    /// a blind test: the lyrics would give the answer.
    pub(super) async fn lyrics<B: Backend>(&self, ip: IpAddr, key: &str, backend: &B) -> Response {
        if !self.allow(ip, key, Action::Search) {
            return Response::error(429, "slow");
        }
        let snapshot = match tokio::time::timeout(BACKEND_TIMEOUT, backend.state()).await {
            Ok(Ok(snapshot)) => snapshot,
            Ok(Err(e)) => return backend_error(&e),
            Err(_) => return Response::error(502, "backend"),
        };
        if snapshot.blind.is_some() {
            return Response::error(409, "game");
        }
        let Some(track) = snapshot.now.as_ref().map(|t| t.id) else {
            let none = serde_json::json!({ "trackId": null, "lyrics": null, "now": now_ms() });
            return Response::json(200, &none);
        };
        // Lock held during the request: guests opening the lyrics at the same time
        // wait for the first answer instead of each querying TIDAL.
        let mut cache = self.lyrics.lock().await;
        let lyrics = match cache.as_ref() {
            Some((id, lyrics)) if *id == track.0 => lyrics.clone(),
            _ => match tokio::time::timeout(BACKEND_TIMEOUT, backend.lyrics(track)).await {
                Ok(Ok(lyrics)) => {
                    let lyrics = lyrics.map(Arc::new);
                    *cache = Some((track.0, lyrics.clone()));
                    // The state now says whether the track has lyrics (button hidden otherwise).
                    self.refresh.notify_one();
                    lyrics
                }
                Ok(Err(e)) => return backend_error(&e),
                Err(_) => return Response::error(502, "backend"),
            },
        };
        drop(cache);
        let body = serde_json::json!({
            "trackId": track.0,
            "lyrics": lyrics.as_deref(),
            "now": now_ms(),
        });
        Response::json(200, &body)
    }

    /// Does the track `track` have lyrics? `None` until someone requests them.
    pub(super) fn known_lyrics(&self, track: u64) -> Option<bool> {
        let cache = self.lyrics.try_lock().ok()?;
        match cache.as_ref() {
            Some((id, lyrics)) if *id == track => Some(lyrics.is_some()),
            _ => None,
        }
    }
}
