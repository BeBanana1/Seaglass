//! Intégrations système : elles reçoivent un résumé de l'état du lecteur (`NowPlaying`)
//! et renvoient leurs actions (touches média…) sous forme de `Cmd` à l'acteur.

#[cfg(windows)]
pub mod smtc;

use crate::player::PlayerState;

/// Intégrations branchées sur le lecteur, notifiées à chaque changement d'état émis.
#[derive(Default)]
pub struct Observers {
    #[cfg(windows)]
    pub smtc: Option<smtc::SmtcHandle>,
}

impl Observers {
    pub fn notify(&self, state: &PlayerState) {
        #[cfg(windows)]
        if let Some(smtc) = &self.smtc {
            smtc.update(NowPlaying::from_player(state));
        }
        #[cfg(not(windows))]
        let _ = state;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaInfo {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub cover_url: Option<String>,
    /// Secondes.
    pub duration: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackStatus {
    Stopped,
    Changing,
    Playing,
    Paused,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NowPlaying {
    pub media: Option<MediaInfo>,
    pub status: PlaybackStatus,
    pub position: f64,
    pub has_next: bool,
    pub has_previous: bool,
}

impl NowPlaying {
    pub fn from_player(s: &PlayerState) -> Self {
        let media = s.track.as_ref().map(|t| MediaInfo {
            title: match &t.version {
                Some(v) => format!("{} ({v})", t.title),
                None => t.title.clone(),
            },
            artist: t
                .artists
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            album: t.album.title.clone(),
            cover_url: t.album.cover_url.clone(),
            duration: if s.duration > 0.0 {
                s.duration
            } else {
                f64::from(t.duration)
            },
        });
        // Un titre restauré (en pause, mpv inactif) reste « en pause » : les touches
        // média peuvent alors relancer la lecture.
        let status = if media.is_none() {
            PlaybackStatus::Stopped
        } else if s.loading || s.buffering {
            PlaybackStatus::Changing
        } else if s.paused {
            PlaybackStatus::Paused
        } else if s.idle {
            PlaybackStatus::Stopped
        } else {
            PlaybackStatus::Playing
        };
        Self {
            media,
            status,
            position: s.time_pos,
            has_next: s.has_next,
            has_previous: s.has_previous || s.track.is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::{AlbumRef, ArtistRef, Track, TrackId};

    fn state() -> PlayerState {
        PlayerState {
            track: Some(Track {
                id: TrackId(1),
                title: "Time".into(),
                version: Some("Live".into()),
                duration: 276,
                artists: vec![
                    ArtistRef {
                        id: 1,
                        name: "A".into(),
                    },
                    ArtistRef {
                        id: 2,
                        name: "B".into(),
                    },
                ],
                album: AlbumRef {
                    id: 3,
                    title: "Inception".into(),
                    cover_url: None,
                    color: None,
                },
                explicit: false,
                hires: true,
            }),
            ..PlayerState::default()
        }
    }

    #[test]
    fn metadata_and_status() {
        let mut s = state();
        let np = NowPlaying::from_player(&s);
        assert_eq!(np.status, PlaybackStatus::Playing);
        let media = np.media.unwrap();
        assert_eq!(media.title, "Time (Live)");
        assert_eq!(media.artist, "A, B");
        assert_eq!(media.duration, 276.0);

        s.paused = true;
        s.idle = true;
        assert_eq!(NowPlaying::from_player(&s).status, PlaybackStatus::Paused);
        s.loading = true;
        assert_eq!(NowPlaying::from_player(&s).status, PlaybackStatus::Changing);
        s.track = None;
        assert_eq!(NowPlaying::from_player(&s).status, PlaybackStatus::Stopped);
    }
}
