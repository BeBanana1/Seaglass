//! Logique pure de MPRIS (#312) : ce que le serveur D-Bus expose et ce qu'il doit signaler,
//! calculé à partir de `NowPlaying`. Compilée partout pour les tests, utilisée sous Linux.

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::time::Instant;

use crate::integrations::{MediaInfo, NowPlaying, PlaybackStatus};
use crate::player::Repeat;

/// Préfixe des `mpris:trackid` : un chemin D-Bus propre à l'app, suivi de l'identifiant.
const TRACK_PATH_PREFIX: &str = "/dev/seaglass/app/track/";
/// `mpris:trackid` réservé par la spécification quand rien n'est chargé.
pub const NO_TRACK: &str = "/org/mpris/MediaPlayer2/TrackList/NoTrack";
/// Écart toléré entre la position reçue et la position estimée avant de signaler un saut.
const SEEK_TOLERANCE: f64 = 1.5;

pub fn track_path(id: u64) -> String {
    format!("{TRACK_PATH_PREFIX}{id}")
}

/// Microsecondes, l'unité des positions MPRIS.
pub fn micros(seconds: f64) -> i64 {
    (seconds.max(0.0) * 1_000_000.0).round() as i64
}

pub fn seconds(micros: i64) -> f64 {
    micros as f64 / 1_000_000.0
}

/// `PlaybackStatus` MPRIS. `Changing` (chargement, mise en mémoire tampon) n'existe pas
/// côté MPRIS : `None`, le statut précédent est gardé.
fn status_str(status: PlaybackStatus) -> Option<&'static str> {
    match status {
        PlaybackStatus::Stopped => Some("Stopped"),
        PlaybackStatus::Changing => None,
        PlaybackStatus::Playing => Some("Playing"),
        PlaybackStatus::Paused => Some("Paused"),
    }
}

pub fn loop_status(repeat: Repeat) -> &'static str {
    match repeat {
        Repeat::Off => "None",
        Repeat::All => "Playlist",
        Repeat::One => "Track",
    }
}

pub fn parse_loop_status(value: &str) -> Option<Repeat> {
    match value {
        "None" => Some(Repeat::Off),
        "Playlist" => Some(Repeat::All),
        "Track" => Some(Repeat::One),
        _ => None,
    }
}

/// Niveau du lecteur (0–100) pour un `Volume` MPRIS (0.0–1.0, au-delà ramené à 1.0).
pub fn volume_level(volume: f64) -> u8 {
    if volume.is_nan() {
        return 0;
    }
    (volume.clamp(0.0, 1.0) * 100.0).round() as u8
}

/// Commande issue de `Seek` ou `SetPosition`, traduite en `Cmd` par le serveur. Pas de
/// `Cmd` ici : en détruire un dans un test tire le code de la zone de notification, et le
/// binaire de test Windows ne démarre plus (comctl32 v6, voir `docs/android.md`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Jump {
    To(f64),
    Next,
}

/// Métadonnées d'un titre, avant leur conversion en dictionnaire D-Bus.
#[derive(Debug, Clone, PartialEq)]
pub struct Metadata {
    pub track_id: String,
    pub length: i64,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub art_url: Option<String>,
}

impl Metadata {
    fn from_media(m: &MediaInfo) -> Self {
        Self {
            track_id: track_path(m.track_id),
            length: micros(m.duration),
            title: m.title.clone(),
            // `NowPlaying` porte les artistes déjà joints : une seule entrée suffit à
            // l'affichage, et `xesam:artist` reste une liste comme l'exige la spécification.
            artists: vec![m.artist.clone()],
            album: m.album.clone(),
            art_url: m.cover_url.clone(),
        }
    }
}

/// Propriétés à signaler par `PropertiesChanged` après une mise à jour, et saut éventuel
/// (signal `Seeked`, en secondes).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Changes {
    pub metadata: bool,
    pub status: bool,
    pub loop_status: bool,
    pub shuffle: bool,
    pub volume: bool,
    /// `CanGoNext`, `CanGoPrevious`, `CanPlay`, `CanPause`, `CanSeek`.
    pub controls: bool,
    pub seeked: Option<f64>,
}

/// Dernier point connu de la position, pour l'estimer entre deux mises à jour.
#[derive(Debug, Clone, Copy)]
struct Clock {
    at: Instant,
    position: f64,
    running: bool,
}

impl Clock {
    fn position_at(&self, now: Instant) -> f64 {
        if self.running {
            self.position + now.saturating_duration_since(self.at).as_secs_f64()
        } else {
            self.position
        }
    }
}

/// État exposé sur le bus, tenu par l'interface `Player`.
#[derive(Debug, Clone)]
pub struct State {
    np: Option<NowPlaying>,
    status: &'static str,
    clock: Option<Clock>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            np: None,
            status: "Stopped",
            clock: None,
        }
    }
}

impl State {
    /// Prend un nouvel état du lecteur et dit ce qui a changé.
    pub fn update(&mut self, np: NowPlaying, now: Instant) -> Changes {
        let status = status_str(np.status).unwrap_or(self.status);
        let last = self.np.as_ref();
        let same_track = last.and_then(|l| l.media.as_ref().map(|m| m.track_id))
            == np.media.as_ref().map(|m| m.track_id);
        let seeked = match (self.clock, same_track && np.media.is_some()) {
            (Some(clock), true)
                if (np.position - clock.position_at(now)).abs() > SEEK_TOLERANCE =>
            {
                Some(np.position)
            }
            _ => None,
        };
        let changes = Changes {
            metadata: last.map(|l| &l.media) != Some(&np.media),
            status: status != self.status,
            loop_status: last.map(|l| l.repeat) != Some(np.repeat),
            shuffle: last.map(|l| l.shuffle) != Some(np.shuffle),
            volume: last.map(|l| l.volume) != Some(np.volume),
            controls: last.map(controls) != Some(controls(&np)),
            seeked,
        };
        self.clock = Some(Clock {
            at: now,
            position: np.position,
            running: status == "Playing",
        });
        self.status = status;
        self.np = Some(np);
        changes
    }

    pub fn status(&self) -> &'static str {
        self.status
    }

    pub fn now_playing(&self) -> Option<&NowPlaying> {
        self.np.as_ref()
    }

    pub fn metadata(&self) -> Option<Metadata> {
        self.media().map(Metadata::from_media)
    }

    fn media(&self) -> Option<&MediaInfo> {
        self.np.as_ref().and_then(|np| np.media.as_ref())
    }

    fn duration(&self) -> f64 {
        self.media().map_or(0.0, |m| m.duration)
    }

    /// Position estimée, bornée à la durée du titre.
    pub fn position(&self, now: Instant) -> f64 {
        let Some(clock) = self.clock.filter(|_| self.media().is_some()) else {
            return 0.0;
        };
        let position = clock.position_at(now);
        let duration = self.duration();
        if duration > 0.0 {
            position.min(duration)
        } else {
            position
        }
    }

    pub fn can_play(&self) -> bool {
        self.media().is_some()
    }

    pub fn can_seek(&self) -> bool {
        self.duration() > 0.0
    }

    pub fn can_go_next(&self) -> bool {
        self.np.as_ref().is_some_and(|np| np.has_next)
    }

    pub fn can_go_previous(&self) -> bool {
        self.np.as_ref().is_some_and(|np| np.has_previous)
    }

    /// `Seek(offset)` : position relative. Au-delà de la fin, la spécification demande de
    /// passer au titre suivant ; avant le début, de revenir à 0.
    pub fn seek(&self, offset: i64, now: Instant) -> Option<Jump> {
        if !self.can_seek() {
            return None;
        }
        let target = self.position(now) + seconds(offset);
        if target >= self.duration() {
            Some(Jump::Next)
        } else {
            Some(Jump::To(target.max(0.0)))
        }
    }

    /// `SetPosition(trackid, position)` : ignoré si le titre a changé entre-temps ou si la
    /// position sort du titre, comme le veut la spécification.
    pub fn set_position(&self, track_id: &str, position: i64) -> Option<Jump> {
        let media = self.media()?;
        let target = seconds(position);
        (self.can_seek()
            && track_id == track_path(media.track_id)
            && (0.0..=media.duration).contains(&target))
        .then_some(Jump::To(target))
    }
}

fn controls(np: &NowPlaying) -> (bool, bool, bool, bool) {
    let duration = np.media.as_ref().map_or(0.0, |m| m.duration);
    (
        np.has_next,
        np.has_previous,
        np.media.is_some(),
        duration > 0.0,
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn np(track_id: u64, status: PlaybackStatus, position: f64) -> NowPlaying {
        NowPlaying {
            media: Some(MediaInfo {
                track_id,
                title: "Time".into(),
                artist: "Pink Floyd".into(),
                album: "The Dark Side of the Moon".into(),
                cover_url: Some("https://resources.tidal.com/images/a/b/640x640.jpg".into()),
                duration: 413.0,
            }),
            status,
            position,
            has_next: true,
            has_previous: true,
            shuffle: false,
            repeat: Repeat::Off,
            volume: 1.0,
        }
    }

    #[test]
    fn first_update_reports_everything() {
        let mut state = State::default();
        let changes = state.update(np(1, PlaybackStatus::Playing, 0.0), Instant::now());
        assert!(changes.metadata && changes.status && changes.loop_status);
        assert!(changes.shuffle && changes.volume && changes.controls);
        assert_eq!(changes.seeked, None);
        assert_eq!(state.status(), "Playing");
        let meta = state.metadata().unwrap();
        assert_eq!(meta.track_id, "/dev/seaglass/app/track/1");
        assert_eq!(meta.length, 413_000_000);
        assert_eq!(meta.artists, vec!["Pink Floyd".to_owned()]);
    }

    #[test]
    fn position_ticks_change_nothing() {
        let t0 = Instant::now();
        let mut state = State::default();
        state.update(np(1, PlaybackStatus::Playing, 10.0), t0);
        let changes = state.update(
            np(1, PlaybackStatus::Playing, 12.0),
            t0 + Duration::from_secs(2),
        );
        assert_eq!(changes, Changes::default());
        let estimated = state.position(t0 + Duration::from_millis(2500));
        assert!((estimated - 12.5).abs() < 1e-6);
    }

    #[test]
    fn jump_emits_seeked_but_not_on_track_change() {
        let t0 = Instant::now();
        let mut state = State::default();
        state.update(np(1, PlaybackStatus::Playing, 10.0), t0);
        let changes = state.update(np(1, PlaybackStatus::Playing, 200.0), t0);
        assert_eq!(changes.seeked, Some(200.0));
        let changes = state.update(np(2, PlaybackStatus::Playing, 0.0), t0);
        assert!(changes.metadata);
        assert_eq!(changes.seeked, None);
    }

    #[test]
    fn paused_position_does_not_advance() {
        let t0 = Instant::now();
        let mut state = State::default();
        state.update(np(1, PlaybackStatus::Paused, 30.0), t0);
        assert_eq!(state.position(t0 + Duration::from_secs(10)), 30.0);
        // Rester en pause 10 s n'est pas un saut.
        let changes = state.update(
            np(1, PlaybackStatus::Paused, 30.0),
            t0 + Duration::from_secs(10),
        );
        assert_eq!(changes.seeked, None);
    }

    #[test]
    fn changing_keeps_previous_status() {
        let mut state = State::default();
        state.update(np(1, PlaybackStatus::Paused, 0.0), Instant::now());
        let changes = state.update(np(1, PlaybackStatus::Changing, 0.0), Instant::now());
        assert!(!changes.status);
        assert_eq!(state.status(), "Paused");
    }

    #[test]
    fn stopped_without_media() {
        let mut state = State::default();
        let stopped = NowPlaying {
            media: None,
            status: PlaybackStatus::Stopped,
            has_next: false,
            has_previous: false,
            ..np(1, PlaybackStatus::Stopped, 0.0)
        };
        state.update(stopped, Instant::now());
        assert_eq!(state.metadata(), None);
        assert!(!state.can_play() && !state.can_seek() && !state.can_go_next());
        assert_eq!(state.position(Instant::now()), 0.0);
        assert!(state.seek(5_000_000, Instant::now()).is_none());
    }

    #[test]
    fn seek_is_relative_and_bounded() {
        let t0 = Instant::now();
        let mut state = State::default();
        state.update(np(1, PlaybackStatus::Paused, 100.0), t0);
        assert_eq!(state.seek(10_000_000, t0), Some(Jump::To(110.0)));
        assert_eq!(state.seek(-500_000_000, t0), Some(Jump::To(0.0)));
        assert_eq!(state.seek(400_000_000, t0), Some(Jump::Next));
    }

    #[test]
    fn set_position_checks_track_and_range() {
        let mut state = State::default();
        state.update(np(7, PlaybackStatus::Playing, 0.0), Instant::now());
        let path = track_path(7);
        assert_eq!(state.set_position(&path, 60_000_000), Some(Jump::To(60.0)));
        assert!(state.set_position(&track_path(8), 60_000_000).is_none());
        assert!(state.set_position(&path, -1).is_none());
        assert!(state.set_position(&path, 500_000_000).is_none());
    }

    #[test]
    fn loop_and_volume_mappings() {
        for repeat in [Repeat::Off, Repeat::All, Repeat::One] {
            assert_eq!(parse_loop_status(loop_status(repeat)), Some(repeat));
        }
        assert_eq!(parse_loop_status("Album"), None);
        assert_eq!(volume_level(0.5), 50);
        assert_eq!(volume_level(1.7), 100);
        assert_eq!(volume_level(-0.2), 0);
        assert_eq!(volume_level(f64::NAN), 0);
    }
}
