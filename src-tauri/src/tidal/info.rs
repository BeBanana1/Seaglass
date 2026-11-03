//! Infos détaillées d'un titre (#3), à la demande : métadonnées de `tracks/{id}` (BPM,
//! tonalité, copyright, position) et format réel de `playbackinfopostpaywall`
//! (profondeur, fréquence, débit du MPD, ReplayGain). Vérifié le 2026-10-30. Crédits :
//! `credits.rs` (#84).
//!
//! Le format exige une requête de lecture par titre : on ne le demande qu'à l'ouverture
//! du panneau, jamais pour une liste. Le segment d'initialisation n'est pas téléchargé.

use serde::Deserialize;

use super::api::{TidalClient, stream_format};
use super::models::{AudioQuality, HiresEdition, TrackId, TrackInfo};
use super::raw::RawTrack;
use super::{Result, TidalError};

/// Qualité demandée, la même que pour une lecture.
const QUALITY: AudioQuality = AudioQuality::HiResLossless;

/// `tracks/{id}` avec les champs que `RawTrack` ignore.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTrackDetails {
    #[serde(flatten)]
    track: RawTrack,
    bpm: Option<f64>,
    key: Option<String>,
    key_scale: Option<String>,
    copyright: Option<String>,
    track_number: Option<u32>,
    volume_number: Option<u32>,
}

impl TidalClient {
    pub async fn track_info(&self, id: TrackId) -> Result<TrackInfo> {
        let raw: RawTrackDetails = self.get(&format!("tracks/{id}"), &[]).await?;
        let tags = raw.track.tags();
        let track = raw
            .track
            .into_track()
            .ok_or_else(|| TidalError::Decode(format!("titre {id} sans album")))?;
        self.cache.insert_all([&track]);

        let (playback, edition, credits) = tokio::join!(
            self.playback(id, QUALITY),
            self.hires_edition(&track),
            self.track_credits(id)
        );
        // Les crédits sont un complément : une erreur ne bloque pas le panneau.
        let credits = credits.unwrap_or_else(|e| {
            tracing::warn!("crédits de {id} illisibles : {e}");
            Vec::new()
        });
        let (stream, stream_error, gains) = match playback {
            Ok((raw, decoded)) => (
                Some(stream_format(&raw, &decoded)),
                None,
                [
                    raw.track_replay_gain,
                    raw.track_peak_amplitude,
                    raw.album_replay_gain,
                    raw.album_peak_amplitude,
                ],
            ),
            Err(e) => (None, Some(e.to_string()), [None; 4]),
        };
        let hires_edition = match edition {
            Ok(Some(edition)) => {
                let stream = match self.playback(edition.id, QUALITY).await {
                    Ok((raw, decoded)) => Some(stream_format(&raw, &decoded)),
                    Err(e) => {
                        tracing::warn!("format de l'édition {} : {e}", edition.id);
                        None
                    }
                };
                Some(HiresEdition {
                    track: edition,
                    stream,
                    played: false,
                })
            }
            Ok(None) => None,
            Err(e) => {
                tracing::warn!("recherche hi-res de {id} impossible : {e}");
                None
            }
        };

        Ok(TrackInfo {
            track,
            tags,
            stream,
            stream_error,
            track_gain: gains[0],
            track_peak: gains[1],
            album_gain: gains[2],
            album_peak: gains[3],
            bpm: raw.bpm.filter(|b| *b > 0.0).map(|b| b.round() as u32),
            key: non_empty(raw.key),
            key_scale: non_empty(raw.key_scale),
            copyright: non_empty(raw.copyright),
            track_number: raw.track_number,
            volume_number: raw.volume_number,
            hires_edition,
            credits,
        })
    }
}

fn non_empty(text: Option<String>) -> Option<String> {
    text.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn details_extend_the_track() {
        let raw: RawTrackDetails = serde_json::from_str(
            r#"{"id":68709105,"title":"Hotel California (2013 Remaster)","duration":391,
                "album":{"id":68709104,"title":"Hotel California (2013 Remaster)"},
                "mediaMetadata":{"tags":["LOSSLESS","HIRES_LOSSLESS"]},"isrc":"USEE11300195",
                "bpm":147,"key":"B","keyScale":"MINOR","copyright":"℗ 1976, 2013 Elektra Records LLC",
                "trackNumber":1,"volumeNumber":1,"popularity":60}"#,
        )
        .unwrap();
        assert_eq!(raw.bpm, Some(147.0));
        assert_eq!(raw.key.as_deref(), Some("B"));
        assert_eq!(raw.key_scale.as_deref(), Some("MINOR"));
        assert_eq!(raw.track_number, Some(1));
        assert_eq!(raw.track.tags(), ["LOSSLESS", "HIRES_LOSSLESS"]);
        let track = raw.track.into_track().unwrap();
        assert!(track.hires);
        assert_eq!(track.isrc.as_deref(), Some("USEE11300195"));
    }

    #[test]
    fn blank_fields_are_dropped() {
        assert_eq!(non_empty(Some("  ".into())), None);
        assert_eq!(non_empty(Some(" B ".into())).as_deref(), Some("B"));
        assert_eq!(non_empty(None), None);
    }
}
