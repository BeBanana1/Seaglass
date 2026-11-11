//! Activité affichée par Discord (#215) : logique pure, sans connexion.
//!
//! « Écoute » (type 2) : titre en `details`, artistes en `state` (repris dans la liste des
//! membres, `status_display_type` = 1), pochette et album, barre de progression par les
//! horodatages de début et de fin. Rien en pause ni à l'arrêt.

use serde_json::{Value, json};

use crate::i18n::tr;
use crate::integrations::{NowPlaying, PlaybackStatus};

/// Longueur maximale des textes acceptée par Discord (`details`, `state`, `large_text`).
const MAX_TEXT: usize = 128;
/// Écart des horodatages au-delà duquel on considère qu'il y a eu un saut.
const SEEK_TOLERANCE_MS: i64 = 2_000;

#[derive(Debug, Clone, PartialEq)]
pub struct Activity {
    pub track_id: u64,
    pub details: String,
    pub state: String,
    pub cover_url: Option<String>,
    pub album: String,
    /// Début et fin de la barre de progression, en ms depuis l'epoch ; absents sans durée.
    pub start_ms: i64,
    pub end_ms: Option<i64>,
    pub button: String,
}

/// Activité voulue pour `np`. Pendant un chargement, l'activité précédente reste : la
/// présence ne clignote pas entre deux titres.
pub fn desired(
    np: Option<&NowPlaying>,
    previous: Option<&Activity>,
    now_ms: i64,
) -> Option<Activity> {
    let np = np?;
    match np.status {
        PlaybackStatus::Changing => return previous.cloned(),
        PlaybackStatus::Paused | PlaybackStatus::Stopped => return None,
        PlaybackStatus::Playing => {}
    }
    let media = np.media.as_ref()?;
    let start_ms = now_ms - (np.position.max(0.0) * 1000.0) as i64;
    let end_ms = (media.duration > 0.0).then(|| start_ms + (media.duration * 1000.0) as i64);
    Some(Activity {
        track_id: media.track_id,
        details: text(&media.title),
        state: text(&media.artist),
        cover_url: media
            .cover_url
            .clone()
            .filter(|u| u.starts_with("https://")),
        album: text(&media.album),
        start_ms,
        end_ms,
        button: tr("Ouvrir dans TIDAL", "Open in TIDAL").to_owned(),
    })
}

/// Vrai si l'activité envoyée doit être remplacée : autre titre ou autre texte, apparition
/// ou disparition, ou saut dans le titre (horodatages décalés de plus de 2 s).
pub fn needs_update(sent: Option<&Activity>, next: Option<&Activity>) -> bool {
    match (sent, next) {
        (None, None) => false,
        (Some(a), Some(b)) => {
            let same_text = Activity {
                start_ms: b.start_ms,
                end_ms: b.end_ms,
                ..a.clone()
            } == *b;
            !same_text
                || (a.start_ms - b.start_ms).abs() > SEEK_TOLERANCE_MS
                || a.end_ms.is_some() != b.end_ms.is_some()
        }
        _ => true,
    }
}

impl Activity {
    pub fn to_json(&self) -> Value {
        let mut activity = json!({
            "type": 2,
            "status_display_type": 1,
            "details": self.details,
            "state": self.state,
            "timestamps": { "start": self.start_ms },
            "buttons": [{
                "label": self.button,
                "url": format!("https://tidal.com/browse/track/{}", self.track_id),
            }],
        });
        if let Some(end) = self.end_ms {
            activity["timestamps"]["end"] = json!(end);
        }
        if let Some(cover) = &self.cover_url {
            activity["assets"] = json!({ "large_image": cover, "large_text": self.album });
        }
        activity
    }
}

/// Coupé à 128 caractères ; Discord refuse aussi un texte de moins de 2 caractères.
fn text(s: &str) -> String {
    let s = s.trim();
    let mut out: String = if s.chars().count() > MAX_TEXT {
        let mut cut: String = s.chars().take(MAX_TEXT - 1).collect();
        cut.push('…');
        cut
    } else {
        s.to_owned()
    };
    while out.chars().count() < 2 {
        out.push('\u{200b}');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::MediaInfo;
    use crate::player::Repeat;

    fn playing(position: f64) -> NowPlaying {
        NowPlaying {
            media: Some(MediaInfo {
                track_id: 42,
                title: "Time".into(),
                artist: "Pink Floyd".into(),
                album: "The Dark Side of the Moon".into(),
                cover_url: Some("https://resources.tidal.com/images/a/b/320x320.jpg".into()),
                duration: 413.0,
            }),
            status: PlaybackStatus::Playing,
            position,
            has_next: false,
            has_previous: false,
            shuffle: false,
            repeat: Repeat::Off,
            volume: 1.0,
        }
    }

    #[test]
    fn listening_activity() {
        let a = desired(Some(&playing(13.0)), None, 100_000).unwrap();
        assert_eq!(
            a.to_json(),
            json!({
                "type": 2,
                "status_display_type": 1,
                "details": "Time",
                "state": "Pink Floyd",
                "timestamps": { "start": 87_000, "end": 500_000 },
                "assets": {
                    "large_image": "https://resources.tidal.com/images/a/b/320x320.jpg",
                    "large_text": "The Dark Side of the Moon",
                },
                "buttons": [{
                    "label": "Ouvrir dans TIDAL",
                    "url": "https://tidal.com/browse/track/42",
                }],
            })
        );
    }

    #[test]
    fn paused_or_stopped_clears() {
        let mut np = playing(0.0);
        np.status = PlaybackStatus::Paused;
        assert_eq!(desired(Some(&np), None, 0), None);
        np.status = PlaybackStatus::Stopped;
        assert_eq!(desired(Some(&np), None, 0), None);
        assert_eq!(desired(None, None, 0), None);
    }

    #[test]
    fn loading_keeps_previous() {
        let previous = desired(Some(&playing(0.0)), None, 0);
        let mut np = playing(0.0);
        np.status = PlaybackStatus::Changing;
        np.media = None;
        assert_eq!(desired(Some(&np), previous.as_ref(), 5_000), previous);
    }

    #[test]
    fn no_cover_no_duration() {
        let mut np = playing(0.0);
        let media = np.media.as_mut().unwrap();
        media.cover_url = None;
        media.duration = 0.0;
        let json = desired(Some(&np), None, 1_000).unwrap().to_json();
        assert!(json.get("assets").is_none());
        assert_eq!(json["timestamps"], json!({ "start": 1_000 }));
    }

    #[test]
    fn update_only_on_change_or_seek() {
        let a = desired(Some(&playing(10.0)), None, 100_000);
        // Lecture qui avance normalement : mêmes horodatages à l'arrondi près.
        let b = desired(Some(&playing(20.2)), None, 110_000);
        assert!(!needs_update(a.as_ref(), b.as_ref()));
        // Saut de 30 s.
        let c = desired(Some(&playing(50.0)), None, 110_000);
        assert!(needs_update(a.as_ref(), c.as_ref()));
        // Autre titre.
        let mut np = playing(10.0);
        np.media.as_mut().unwrap().track_id = 7;
        assert!(needs_update(
            a.as_ref(),
            desired(Some(&np), None, 100_000).as_ref()
        ));
        assert!(needs_update(a.as_ref(), None));
        assert!(needs_update(None, a.as_ref()));
        assert!(!needs_update(None, None));
    }

    #[test]
    fn text_limits() {
        assert_eq!(text("A"), "A\u{200b}");
        let long = "x".repeat(200);
        let cut = text(&long);
        assert_eq!(cut.chars().count(), 128);
        assert!(cut.ends_with('…'));
    }
}
