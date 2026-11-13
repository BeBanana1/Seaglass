//! Ajouts et retraits des invités (#223), sous les règles de l'hôte (#288) : doublons,
//! limite par invité, albums, ajouts suspendus ; plafond de la Jam dans la file (#256).

use std::net::IpAddr;

use serde::Deserialize;

use super::{BACKEND_TIMEOUT, Backend, Core, MAX_ALBUM_TRACKS, backend_error};
use crate::jam::guard::Action;
use crate::jam::guests::{sanitize_name, sanitize_token};
use crate::jam::http::Response;
use crate::jam::rules;
use crate::tidal::models::TrackId;

impl Core {
    pub(super) async fn add<B: Backend>(
        &self,
        body: &[u8],
        ip: IpAddr,
        key: &str,
        backend: &B,
    ) -> Response {
        let Ok(body) = serde_json::from_slice::<AddBody>(body) else {
            return Response::error(400, "body");
        };
        if !self.adds_open() {
            return Response::error(403, "closed");
        }
        let offered = self
            .session
            .lock()
            .map(|s| s.offered.contains(body.track_id))
            .unwrap_or(false);
        if !offered {
            return Response::error(400, "unknown");
        }
        if !self.allow(ip, key, Action::Add) {
            return Response::error(429, "slow");
        }
        let _turn = self.add_turn.lock().await;
        let ids = match self.admit(&[body.track_id], key, backend).await {
            Ok(ids) => ids,
            Err(response) => return response,
        };
        if let Err(e) = backend
            .enqueue(ids.iter().copied().map(TrackId).collect())
            .await
        {
            return backend_error(&e);
        }
        let batch = self.note(&ids, &body.guest, key, ip, backend);
        Response::json(200, &serde_json::json!({ "ok": true, "batch": batch }))
    }

    /// Titres de l'ajout que les règles laissent passer (doublons, limite par invité), d'après
    /// la file manuelle du moment.
    async fn admit<B: Backend>(
        &self,
        candidates: &[u64],
        key: &str,
        backend: &B,
    ) -> Result<Vec<u64>, Response> {
        let snapshot = match tokio::time::timeout(BACKEND_TIMEOUT, backend.state()).await {
            Ok(Ok(snapshot)) => snapshot,
            Ok(Err(e)) => return Err(backend_error(&e)),
            Err(_) => return Err(backend_error("lecteur sans réponse")),
        };
        // Blind test avec les invités (#441) : la file attend la fin de la partie.
        if snapshot.jam_game().is_some() {
            return Err(Response::error(403, "game"));
        }
        let manual: Vec<u64> = snapshot.manual.iter().map(|t| t.id.0).collect();
        let (pending, total) = self
            .guests
            .lock()
            .map(|g| (g.pending(key, &manual), g.pending_total(&manual)))
            .unwrap_or((0, 0));
        let room = self.rules().room(pending);
        rules::admit(candidates, &manual, room, rules::jam_room(total)).map_err(|refusal| {
            let (status, kind) = refusal.kind();
            Response::error(status, kind)
        })
    }

    /// Les titres d'un album à la fin de la file, en un seul lot (#223).
    pub(super) async fn add_album<B: Backend>(
        &self,
        body: &[u8],
        ip: IpAddr,
        key: &str,
        backend: &B,
    ) -> Response {
        let Ok(body) = serde_json::from_slice::<AddAlbumBody>(body) else {
            return Response::error(400, "body");
        };
        if !self.adds_open() {
            return Response::error(403, "closed");
        }
        if !self.rules().albums {
            return Response::error(403, "albums");
        }
        let offered = self
            .session
            .lock()
            .map(|s| s.offered_albums.contains(body.album_id))
            .unwrap_or(false);
        if !offered {
            return Response::error(400, "unknown");
        }
        if !self.allow(ip, key, Action::Add) {
            return Response::error(429, "slow");
        }
        let tracks = match backend.album_tracks(body.album_id).await {
            Ok(tracks) => tracks,
            Err(e) => return backend_error(&e),
        };
        let ids: Vec<u64> = tracks
            .iter()
            .map(|t| t.id.0)
            .take(MAX_ALBUM_TRACKS)
            .collect();
        if ids.is_empty() {
            return Response::error(400, "empty");
        }
        // Les titres viennent de TIDAL avant de prendre son tour : un album lent à venir ne
        // retient pas les ajouts des autres.
        let _turn = self.add_turn.lock().await;
        let ids = match self.admit(&ids, key, backend).await {
            Ok(ids) => ids,
            Err(response) => return response,
        };
        if let Err(e) = backend
            .enqueue(ids.iter().copied().map(TrackId).collect())
            .await
        {
            return backend_error(&e);
        }
        let batch = self.note(&ids, &body.guest, key, ip, backend);
        Response::json(
            200,
            &serde_json::json!({ "ok": true, "batch": batch, "count": ids.len() }),
        )
    }

    /// Note un ajout sous le nom et la clé de l'invité (`owner_key`) ; renvoie le lot.
    fn note<B: Backend>(
        &self,
        ids: &[u64],
        guest: &GuestFields,
        key: &str,
        ip: IpAddr,
        backend: &B,
    ) -> Option<u32> {
        let name = guest.name.as_deref().and_then(sanitize_name);
        let batch = {
            let mut guests = self.guests.lock().ok()?;
            let batch = guests.new_batch();
            for id in ids {
                guests.note(*id, batch, name.clone(), Some(key.to_owned()));
            }
            let count = u32::try_from(ids.len()).unwrap_or(u32::MAX);
            guests.members.added(key, name, ip, count);
            backend.guests_changed(&guests);
            batch
        };
        self.refresh.notify_one();
        Some(batch)
    }

    /// Retire un lot de l'invité, ou un seul de ses titres (#223). Seul l'auteur du lot,
    /// reconnu par son jeton, peut le faire.
    pub(super) async fn remove<B: Backend>(
        &self,
        body: &[u8],
        ip: IpAddr,
        key: &str,
        backend: &B,
    ) -> Response {
        let Ok(body) = serde_json::from_slice::<RemoveBody>(body) else {
            return Response::error(400, "body");
        };
        let Some(owner) = sanitize_token(&body.guest) else {
            return Response::error(400, "body");
        };
        if !self.allow(ip, key, Action::Remove) {
            return Response::error(429, "slow");
        }
        let ids = {
            let Ok(mut guests) = self.guests.lock() else {
                return Response::error(502, "backend");
            };
            let ids = guests.take(&owner, body.batch, body.track_id);
            if !ids.is_empty() {
                backend.guests_changed(&guests);
            }
            ids
        };
        if !ids.is_empty() {
            if let Err(e) = backend
                .remove(ids.iter().copied().map(TrackId).collect())
                .await
            {
                return backend_error(&e);
            }
            self.refresh.notify_one();
        }
        Response::json(
            200,
            &serde_json::json!({ "ok": true, "removed": ids.len() }),
        )
    }
}

/// Prénom affiché, facultatif.
#[derive(Deserialize)]
struct GuestFields {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddBody {
    track_id: u64,
    #[serde(flatten)]
    guest: GuestFields,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddAlbumBody {
    album_id: u64,
    #[serde(flatten)]
    guest: GuestFields,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoveBody {
    guest: String,
    batch: u32,
    /// Un seul titre du lot ; tout le lot sinon.
    #[serde(default)]
    track_id: Option<u64>,
}
