//! Transfert de la lecture (#414) : l'app Android d'un téléphone lié reprend ce qui joue
//! sur le PC (`take`), ou le lui rend (`give`). Mêmes code et limites que la télécommande,
//! servi seulement si « Accepter les transferts » est activé.

use std::net::IpAddr;

use crate::jam::guard::Action;
use crate::jam::http::Response;
use crate::player::Handoff;

use super::{Owner, Remote, backend_error};

/// Routes du transfert ; `None` si `path` n'en est pas une.
pub(super) async fn route<R: Remote>(
    owner: &Owner,
    path: &str,
    body: &[u8],
    ip: IpAddr,
    remote: &R,
) -> Option<Response> {
    if !matches!(path, "/api/handoff/take" | "/api/handoff/give") {
        return None;
    }
    if !owner.handoff_enabled() {
        return Some(Response::error(403, "handoff"));
    }
    if !owner.allow(ip, Action::Command) {
        return Some(Response::error(429, "slow"));
    }
    Some(if path == "/api/handoff/take" {
        take(remote).await
    } else {
        give(owner, body, remote).await
    })
}

/// L'état de lecture du PC, qui se met en pause ; 409 `idle` si rien n'est en cours.
async fn take<R: Remote>(remote: &R) -> Response {
    match remote.take_handoff().await {
        Ok(Some(handoff)) => match serde_json::to_string(&handoff) {
            Ok(json) => Response::new(200, "application/json", json),
            Err(e) => backend_error(&e.to_string()),
        },
        Ok(None) => Response::error(409, "idle"),
        Err(e) => backend_error(&e),
    }
}

/// Le PC reprend la lecture envoyée par le téléphone.
async fn give<R: Remote>(owner: &Owner, body: &[u8], remote: &R) -> Response {
    let Ok(handoff) = serde_json::from_slice::<Handoff>(body) else {
        return Response::error(400, "body");
    };
    let Some(handoff) = handoff.received(crate::commands::settings::pins::is_uuid) else {
        return Response::error(400, "body");
    };
    match remote.give_handoff(handoff).await {
        Ok(true) => {
            owner.refresh.notify_one();
            Response::json(200, &serde_json::json!({ "ok": true }))
        }
        Ok(false) => Response::error(400, "body"),
        Err(e) => backend_error(&e),
    }
}
