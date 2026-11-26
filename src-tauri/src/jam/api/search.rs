//! Guest search (#25): TIDAL tracks and albums, or those of a pasted link (#229). What it
//! returns is noted as offered, so that only these can be added afterwards.

use std::net::IpAddr;

use serde::Deserialize;

use super::{Backend, Core, MAX_QUERY_CHARS, backend_error};
use crate::jam::guard::Action;
use crate::jam::http::Response;
use crate::jam::view::{GuestAlbum, GuestTrack};

const MAX_RESULTS: usize = 30;
const MAX_ALBUM_RESULTS: usize = 20;

impl Core {
    /// Titres seuls (tableau) pour une page d'avant #223, titres et albums sinon.
    pub(super) async fn search<B: Backend>(
        &self,
        body: &[u8],
        ip: IpAddr,
        key: &str,
        backend: &B,
    ) -> Response {
        let Ok(body) = serde_json::from_slice::<SearchBody>(body) else {
            return Response::error(400, "body");
        };
        let query = body.query.trim();
        if query.is_empty() {
            return if body.albums {
                Response::json(200, &serde_json::json!({ "tracks": [], "albums": [] }))
            } else {
                Response::json(200, &serde_json::json!([]))
            };
        }
        if query.chars().count() > MAX_QUERY_CHARS {
            return Response::error(400, "query");
        }
        if !self.allow(ip, key, Action::Search) {
            return Response::error(429, "slow");
        }
        // Un lien d'un autre service (#229) : ses titres ou son album, pas une recherche.
        let link = crate::foreign::find(query).is_some();
        let hits = if link {
            backend.link(query.to_owned()).await
        } else {
            backend.search(query.to_owned()).await
        };
        let hits = match hits {
            Ok(hits) => hits,
            Err(e) => return backend_error(&e),
        };
        let tracks: Vec<GuestTrack> = hits
            .tracks
            .iter()
            .take(MAX_RESULTS)
            .map(GuestTrack::from)
            .collect();
        let albums: Vec<GuestAlbum> = if body.albums {
            hits.albums
                .iter()
                .take(MAX_ALBUM_RESULTS)
                .map(GuestAlbum::from)
                .collect()
        } else {
            Vec::new()
        };
        if let Ok(mut s) = self.session.lock() {
            s.offered.note(tracks.iter().map(|t| t.id));
            s.offered_albums.note(albums.iter().map(|a| a.id));
        }
        if body.albums {
            let hits = serde_json::json!({ "tracks": tracks, "albums": albums, "link": link });
            Response::json(200, &hits)
        } else {
            Response::json(200, &serde_json::json!(tracks))
        }
    }
}

#[derive(Deserialize)]
struct SearchBody {
    query: String,
    /// Page qui sait afficher les albums (#223).
    #[serde(default)]
    albums: bool,
}
