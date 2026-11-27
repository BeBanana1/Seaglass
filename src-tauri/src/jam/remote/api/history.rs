//! Listening history synced with the linked phone (#430): same code, same limits and same
//! switch ("Accept handoffs") as the playback handoff.

use std::net::IpAddr;

use crate::handoff::history::SyncRequest;
use crate::jam::guard::Action;
use crate::jam::http::Response;

use super::{Owner, Remote, backend_error};

pub(in crate::jam::remote) const PATH: &str = "/api/plays/sync";

/// History route; `None` if `path` is not one.
pub(super) async fn route<R: Remote>(
    owner: &Owner,
    path: &str,
    body: &[u8],
    ip: IpAddr,
    remote: &R,
) -> Option<Response> {
    if path != PATH {
        return None;
    }
    if !owner.handoff_enabled() {
        return Some(Response::error(403, "handoff"));
    }
    if !owner.allow(ip, Action::Command) {
        return Some(Response::error(429, "slow"));
    }
    let Some(request) = serde_json::from_slice::<SyncRequest>(body)
        .ok()
        .and_then(SyncRequest::received)
    else {
        return Some(Response::error(400, "body"));
    };
    Some(match remote.sync_plays(request).await {
        Ok(response) => match serde_json::to_string(&response) {
            Ok(json) => Response::new(200, "application/json", json),
            Err(e) => backend_error(&e.to_string()),
        },
        Err(e) => backend_error(&e),
    })
}
