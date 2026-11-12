//! Intégrations système : elles reçoivent un résumé de l'état du lecteur (`NowPlaying`)
//! et renvoient leurs actions (touches média…) sous forme de `Cmd` à l'acteur.

// Hors Windows, Linux et Android (macOS…), aucune intégration ne consomme `NowPlaying` :
// sans cela, clippy `-D warnings` échouerait sur du code mort.
#![cfg_attr(
    not(any(windows, target_os = "linux", target_os = "android")),
    allow(dead_code)
)]

#[cfg(desktop)]
pub mod discord;
#[cfg(any(target_os = "android", test))]
pub mod media_session;
#[cfg(any(target_os = "linux", test))]
pub mod mpris;
#[cfg(windows)]
pub mod smtc;
#[cfg(windows)]
pub mod thumbbar;
#[cfg(desktop)]
pub mod tray;

use std::sync::atomic::{AtomicBool, Ordering};

use crate::player::{PlayerState, Repeat};

/// Un appui sur la notification d'Android demande « En lecture » (#163). Gardée jusqu'à ce
/// que l'UI la prenne : rouverte après un balayage des récents, la WebView n'existe pas
/// encore quand la demande arrive.
static NOW_PLAYING_REQUEST: AtomicBool = AtomicBool::new(false);

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn request_now_playing() {
    NOW_PLAYING_REQUEST.store(true, Ordering::Relaxed);
}

/// Vrai une seule fois par demande.
pub fn take_now_playing_request() -> bool {
    NOW_PLAYING_REQUEST.swap(false, Ordering::Relaxed)
}

/// Mode conduite demandé par le lien `seaglass://drive` ou le raccourci de l'app (#193),
/// gardé comme « En lecture » jusqu'à ce que l'UI le prenne.
static DRIVE_REQUEST: AtomicBool = AtomicBool::new(false);

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn request_drive() {
    DRIVE_REQUEST.store(true, Ordering::Relaxed);
}

/// Vrai une seule fois par demande.
pub fn take_drive_request() -> bool {
    DRIVE_REQUEST.swap(false, Ordering::Relaxed)
}

/// Intégrations branchées sur le lecteur, notifiées à chaque changement d'état émis.
#[derive(Default)]
pub struct Observers {
    #[cfg(windows)]
    pub smtc: Option<smtc::SmtcHandle>,
    #[cfg(windows)]
    pub thumbbar: Option<thumbbar::ThumbBarHandle>,
    #[cfg(target_os = "linux")]
    pub mpris: Option<mpris::MprisHandle>,
    #[cfg(desktop)]
    pub tray: Option<tray::TrayHandle>,
    #[cfg(desktop)]
    pub discord: Option<discord::DiscordHandle>,
    #[cfg(target_os = "android")]
    pub media_session: Option<media_session::MediaSessionHandle>,
}

impl Observers {
    pub fn notify(&self, state: &PlayerState) {
        #[cfg(windows)]
        if let Some(smtc) = &self.smtc {
            smtc.update(NowPlaying::from_player(state));
        }
        #[cfg(windows)]
        if let Some(thumbbar) = &self.thumbbar {
            thumbbar.update(state);
        }
        #[cfg(target_os = "linux")]
        if let Some(mpris) = &self.mpris {
            mpris.update(NowPlaying::from_player(state));
        }
        #[cfg(desktop)]
        if let Some(tray) = &self.tray {
            tray.update(state);
        }
        #[cfg(desktop)]
        if let Some(discord) = &self.discord {
            discord.update(NowPlaying::from_player(state));
        }
        #[cfg(target_os = "android")]
        if let Some(session) = &self.media_session {
            session.update(NowPlaying::from_player(state));
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaInfo {
    pub track_id: u64,
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
    pub shuffle: bool,
    pub repeat: Repeat,
    /// 0.0–1.0, 0 quand le son est coupé (MPRIS).
    pub volume: f64,
}

impl NowPlaying {
    pub fn from_player(s: &PlayerState) -> Self {
        let media = s.track.as_ref().map(|t| MediaInfo {
            track_id: t.id.0,
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
            shuffle: s.shuffle,
            repeat: s.repeat,
            volume: if s.volume.muted {
                0.0
            } else {
                f64::from(s.volume.level) / 100.0
            },
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
                    cover_small_url: None,
                    cover_large_url: None,
                    color: None,
                },
                explicit: false,
                ai: false,
                hires: true,
                isrc: None,
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
