//! What the remote control page receives (#336): the player state, the queue, the search
//! results and the playlists, nothing else leaves the app.

use serde::Serialize;

use crate::jam::view::GuestTrack;
use crate::player::{PlayerState, QueueView, Repeat};
use crate::tidal::models::PlaylistSummary;

/// Tracks shown for each part of the queue (manual, rest of the context).
pub const MAX_QUEUE: usize = 50;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteView {
    pub now: Option<GuestTrack>,
    pub paused: bool,
    /// The current track is loading (metadata, stream).
    pub loading: bool,
    /// Position in whole seconds: the state only changes once per second during playback,
    /// the page interpolates between two sends.
    pub position: u32,
    pub duration: u32,
    pub volume: u8,
    pub muted: bool,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub has_previous: bool,
    pub has_next: bool,
    /// Manual queue ("Play next", "Add to queue"), `MAX_QUEUE` at most.
    pub manual: Vec<GuestTrack>,
    /// Rest of the context (album, playlist…), `MAX_QUEUE` at most.
    pub upcoming: Vec<GuestTrack>,
}

impl RemoteView {
    pub fn new(player: &PlayerState, queue: &QueueView) -> Self {
        let tracks = |list: &[crate::tidal::models::Track]| {
            list.iter().take(MAX_QUEUE).map(GuestTrack::from).collect()
        };
        Self {
            now: player.track.as_ref().map(GuestTrack::from),
            paused: player.paused,
            loading: player.loading,
            position: whole_seconds(player.time_pos),
            duration: whole_seconds(player.duration),
            volume: player.volume.level,
            muted: player.volume.muted,
            shuffle: player.shuffle,
            repeat: player.repeat,
            has_previous: player.has_previous,
            has_next: player.has_next,
            manual: tracks(&queue.manual),
            upcoming: tracks(&queue.upcoming),
        }
    }
}

fn whole_seconds(s: f64) -> u32 {
    if s.is_finite() && s > 0.0 {
        // Bounded to `u32`: an absurd duration does not overflow the conversion.
        s.min(f64::from(u32::MAX)).floor() as u32
    } else {
        0
    }
}

/// Playlist from a search or from "My playlists".
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemotePlaylist {
    pub uuid: String,
    pub title: String,
    pub cover: Option<String>,
    pub tracks: Option<u32>,
}

impl From<&PlaylistSummary> for RemotePlaylist {
    fn from(p: &PlaylistSummary) -> Self {
        Self {
            uuid: p.uuid.clone(),
            title: p.title.clone(),
            cover: p.image_url.clone(),
            tracks: p.number_of_tracks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jam::testing::track;

    #[test]
    fn the_view_rounds_the_position_and_bounds_the_queue() {
        let mut player = PlayerState {
            track: Some(track(7)),
            time_pos: 61.9,
            duration: 200.0,
            ..PlayerState::default()
        };
        player.volume.level = 40;
        let queue = QueueView {
            manual: vec![track(1)],
            upcoming: (10..100).map(track).collect(),
            ..QueueView::default()
        };
        let view = RemoteView::new(&player, &queue);
        assert_eq!(view.now.as_ref().map(|t| t.id), Some(7));
        assert_eq!((view.position, view.duration, view.volume), (61, 200, 40));
        assert_eq!(view.manual.len(), 1);
        assert_eq!(view.upcoming.len(), MAX_QUEUE);
        assert_eq!(whole_seconds(f64::NAN), 0);
        assert_eq!(whole_seconds(-3.0), 0);
    }
}
