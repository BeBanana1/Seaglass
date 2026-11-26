//! Commandes du propriétaire lues depuis une requête : celles qui ne demandent rien à TIDAL,
//! et la lecture d'un album ou d'une playlist entiers.

use serde::Deserialize;

use super::{Command, MAX_CONTEXT, Remote, backend_error, parse};
use crate::jam::http::Response;
use crate::player::volume::Volume;
use crate::player::{PlaySource, QueuePart, Repeat};
use crate::tidal::models::TrackId;

/// Commande qui ne demande rien à TIDAL ; `Ok(None)` si `path` n'en est pas une.
pub(super) fn command(path: &str, body: &[u8]) -> Result<Option<Command>, &'static str> {
    let command = match path {
        "/api/pause" => Command::SetPaused(parse::<PausedBody>(body)?.paused),
        "/api/next" => Command::Next,
        "/api/previous" => Command::Previous,
        "/api/seek" => {
            let seconds = parse::<SeekBody>(body)?.seconds;
            if !seconds.is_finite() || seconds < 0.0 {
                return Err("body");
            }
            Command::Seek(seconds)
        }
        "/api/volume" => {
            let level = parse::<VolumeBody>(body)?.level;
            Command::Volume(Volume::validate(level).map_err(|_| "body")?)
        }
        "/api/mute" => Command::Muted(parse::<MutedBody>(body)?.muted),
        "/api/shuffle" => Command::Shuffle(parse::<EnabledBody>(body)?.enabled),
        "/api/repeat" => Command::Repeat(parse::<RepeatBody>(body)?.mode),
        "/api/jump" => {
            let b = parse::<JumpBody>(body)?;
            Command::Jump {
                part: b.part,
                index: b.index,
                id: TrackId(b.track_id),
            }
        }
        "/api/remove" => Command::Remove(TrackId(parse::<TrackBody>(body)?.track_id)),
        "/api/play" => Command::Play {
            ids: vec![TrackId(parse::<TrackBody>(body)?.track_id)],
            shuffle: None,
            source: None,
        },
        "/api/enqueue" => {
            let b = parse::<TrackBody>(body)?;
            Command::Enqueue {
                ids: vec![TrackId(b.track_id)],
                next: b.next,
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(command))
}

/// Album ou playlist entiers, lus depuis le début (ou mélangés).
pub(super) async fn context<R: Remote>(
    path: &str,
    body: &[u8],
    remote: &R,
) -> Result<Command, Response> {
    let (tracks, source, shuffle) = if path == "/api/play-album" {
        let b = parse::<AlbumBody>(body).map_err(|kind| Response::error(400, kind))?;
        let (title, tracks) = remote
            .album(b.album_id)
            .await
            .map_err(|e| backend_error(&e))?;
        let source = PlaySource::Album {
            id: b.album_id,
            title,
        };
        (tracks, source, b.shuffle)
    } else {
        let b = parse::<PlaylistBody>(body).map_err(|kind| Response::error(400, kind))?;
        if !crate::commands::settings::pins::is_uuid(&b.uuid) {
            return Err(Response::error(400, "body"));
        }
        let (title, tracks) = remote
            .playlist(b.uuid.clone())
            .await
            .map_err(|e| backend_error(&e))?;
        let source = PlaySource::Playlist {
            uuid: b.uuid,
            title,
        };
        (tracks, source, b.shuffle)
    };
    if tracks.is_empty() {
        return Err(Response::error(400, "empty"));
    }
    Ok(Command::Play {
        ids: tracks.iter().take(MAX_CONTEXT).map(|t| t.id).collect(),
        // "Play" turns shuffle off, "Shuffle" turns it on (#526).
        shuffle: Some(shuffle),
        source: Some(source),
    })
}

#[derive(Deserialize)]
struct PausedBody {
    paused: bool,
}

#[derive(Deserialize)]
struct SeekBody {
    seconds: f64,
}

#[derive(Deserialize)]
struct VolumeBody {
    level: u8,
}

#[derive(Deserialize)]
struct MutedBody {
    muted: bool,
}

#[derive(Deserialize)]
struct EnabledBody {
    enabled: bool,
}

#[derive(Deserialize)]
struct RepeatBody {
    mode: Repeat,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JumpBody {
    part: QueuePart,
    index: usize,
    track_id: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackBody {
    track_id: u64,
    /// `enqueue` : en tête de la file manuelle (« Lire ensuite ») plutôt qu'à la fin.
    #[serde(default)]
    next: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AlbumBody {
    album_id: u64,
    #[serde(default)]
    shuffle: bool,
}

#[derive(Deserialize)]
struct PlaylistBody {
    uuid: String,
    #[serde(default)]
    shuffle: bool,
}
