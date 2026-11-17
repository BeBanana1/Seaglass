//! Paroles du titre en cours pour les invités (#448). Jamais d'identifiant en paramètre :
//! la Jam ne devient pas un relais vers TIDAL pour n'importe quel titre. Une requête à
//! TIDAL par titre, quel que soit le nombre d'invités qui ouvrent les paroles.

use std::net::IpAddr;
use std::sync::Arc;

use super::{BACKEND_TIMEOUT, Backend, Core, backend_error};
use crate::jam::clock::now_ms;
use crate::jam::guard::Action;
use crate::jam::http::Response;
use crate::tidal::models::Lyrics;

/// Paroles du dernier titre demandé ; `None` = le titre n'en a pas.
pub(super) type CachedLyrics = (u64, Option<Arc<Lyrics>>);

impl Core {
    /// `POST api/lyrics {guest}` : `{trackId, lyrics, now}`, `now` étant l'heure de l'hôte
    /// (la page corrige ainsi l'écart de son horloge pour suivre les lignes). Refusé pendant
    /// un blind test : les paroles donneraient la réponse.
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
        // Verrou tenu pendant la requête : les invités qui ouvrent les paroles en même temps
        // attendent la première réponse au lieu d'interroger TIDAL chacun.
        let mut cache = self.lyrics.lock().await;
        let lyrics = match cache.as_ref() {
            Some((id, lyrics)) if *id == track.0 => lyrics.clone(),
            _ => match tokio::time::timeout(BACKEND_TIMEOUT, backend.lyrics(track)).await {
                Ok(Ok(lyrics)) => {
                    let lyrics = lyrics.map(Arc::new);
                    *cache = Some((track.0, lyrics.clone()));
                    // L'état dit maintenant si le titre a des paroles (bouton masqué sinon).
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

    /// Le titre `track` a-t-il des paroles ? `None` tant que personne ne les a demandées.
    pub(super) fn known_lyrics(&self, track: u64) -> Option<bool> {
        let cache = self.lyrics.try_lock().ok()?;
        match cache.as_ref() {
            Some((id, lyrics)) if *id == track => Some(lyrics.is_some()),
            _ => None,
        }
    }
}
