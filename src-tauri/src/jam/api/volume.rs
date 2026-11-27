//! Volume set by guests (#449), if the host allows it, never above the cap
//! the host chose. Same path as the remote control (#336): `Cmd::SetVolume`.

use std::net::IpAddr;

use serde::Deserialize;

use super::{Backend, Core, backend_error};
use crate::jam::guard::Action;
use crate::jam::http::Response;
use crate::player::volume::Volume;

#[derive(Deserialize)]
struct VolumeBody {
    level: u8,
}

impl Core {
    /// `POST api/volume {guest, level}`: level brought down to the host's cap. Refused if
    /// the host has muted: a guest does not restore it in their place.
    pub(super) async fn volume<B: Backend>(
        &self,
        body: &[u8],
        ip: IpAddr,
        key: &str,
        backend: &B,
    ) -> Response {
        let rules = self.rules();
        if !rules.guest_volume {
            return Response::error(403, "volume");
        }
        let Ok(body) = serde_json::from_slice::<VolumeBody>(body) else {
            return Response::error(400, "body");
        };
        let Ok(level) = Volume::validate(body.level) else {
            return Response::error(400, "body");
        };
        if !self.allow(ip, key, Action::Volume) {
            return Response::error(429, "slow");
        }
        match backend.state().await {
            Ok(snapshot) if snapshot.volume.muted => return Response::error(409, "muted"),
            Ok(_) => {}
            Err(e) => return backend_error(&e),
        }
        let level = rules.guest_level(level);
        if let Err(e) = backend.set_volume(level).await {
            return backend_error(&e);
        }
        self.refresh.notify_one();
        Response::json(200, &serde_json::json!({ "level": level }))
    }
}
