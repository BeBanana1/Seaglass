//! Choice of a track's stream: its own, or that of another hi-res edition of the same
//! recording (#18). Runs outside the actor, in the network resolution.
//!
//! - Next track (preload 30 s before the end): lookup then stream, one after the other; the
//!   time spent is invisible.
//! - Current track: the lookup runs in parallel with the original stream and cannot delay
//!   the start by more than `CURRENT_LOOKUP_LIMIT`. An answer already in the cache avoids
//!   requesting the original stream.
//! - "Ask" mode (#59): the original track starts without waiting, `find_offer` then looks
//!   for the edition to offer.

use std::time::Duration;

use super::actor::{Cmd, Player};
use super::settings::HiresUpgrade;
use crate::tidal::TidalClient;
use crate::tidal::models::{AudioQuality, Stream, StreamFormat, StreamUse, Track};

/// Maximum time given to the search for a hi-res edition for the current track.
const CURRENT_LOOKUP_LIMIT: Duration = Duration::from_millis(1500);

/// Stream to play for `track`. `current`: the track is wanted right away (entry 0).
pub(super) async fn resolve_stream(
    tidal: &TidalClient,
    track: &Track,
    quality: AudioQuality,
    upgrade: bool,
    current: bool,
) -> crate::tidal::Result<Stream> {
    let use_ = if current {
        StreamUse::Play
    } else {
        StreamUse::Preload
    };
    if !upgrade || track.hires || quality != AudioQuality::HiResLossless {
        return tidal.stream(track.id, quality, use_).await;
    }
    let (original, edition) = match tidal.known_hires_edition(track.id) {
        Some(known) => (None, known),
        None if current => {
            let lookup = tokio::time::timeout(CURRENT_LOOKUP_LIMIT, tidal.hires_edition(track));
            let (original, edition) = tokio::join!(tidal.stream(track.id, quality, use_), lookup);
            let edition = match edition {
                Ok(found) => found,
                Err(_) => {
                    tracing::warn!("hi-res lookup for {} too slow, ignored", track.id);
                    Ok(None)
                }
            };
            (Some(original), lookup_result(track, edition))
        }
        None => (None, lookup_result(track, tidal.hires_edition(track).await)),
    };
    if let Some(edition) = edition {
        let base = original.as_ref().and_then(|r| r.as_ref().ok());
        match tidal.stream(edition.id, quality, use_).await {
            Ok(mut stream) if is_better(&stream.format, base.map(|s| &s.format)) => {
                tracing::info!(
                    "{}: hi-res edition {} ({})",
                    track.id,
                    edition.id,
                    edition.album.title
                );
                stream.format.hires_edition = Some((&edition).into());
                return Ok(stream);
            }
            Ok(stream) => tracing::info!(
                "{}: edition {} not better ({:?}/{:?}), original version",
                track.id,
                edition.id,
                stream.format.bit_depth,
                stream.format.sample_rate
            ),
            Err(e) => tracing::warn!("{}: stream of edition {}: {e}", track.id, edition.id),
        }
    }
    match original {
        Some(result) => result,
        None => tidal.stream(track.id, quality, use_).await,
    }
}

/// Hi-res edition to offer for `track`, played in the `original` format: its format (with
/// the album in `hires_edition`) if it is really better. Nothing is played: the stream will
/// be requested if the offer is accepted.
pub(super) async fn find_offer(
    tidal: &TidalClient,
    track: &Track,
    quality: AudioQuality,
    original: &StreamFormat,
) -> Option<StreamFormat> {
    if track.hires || quality != AudioQuality::HiResLossless {
        return None;
    }
    let edition = lookup_result(track, tidal.hires_edition(track).await)?;
    match tidal.playback_format(edition.id, quality).await {
        Ok(mut format) if is_better(&format, Some(original)) => {
            format.hires_edition = Some((&edition).into());
            Some(format)
        }
        Ok(_) => None,
        Err(e) => {
            tracing::warn!("{}: format of edition {}: {e}", track.id, edition.id);
            None
        }
    }
}

impl Player {
    /// "Ask" mode (#59): looks up in the background the hi-res edition to offer for the
    /// current track, unless its album already got an answer. Removes any previous offer
    /// (other track, other setting).
    pub(super) fn maybe_offer_hires(&mut self) {
        self.offer_req += 1;
        self.state.hires_offer = None;
        // Not during a blind test (#434): the banner would name the album.
        if self.hires_upgrade != HiresUpgrade::Ask || !self.loaded || self.blind.is_some() {
            return;
        }
        let (Some(track), Some(format)) = (&self.state.track, &self.state.stream) else {
            return;
        };
        if track.hires
            || format.hires_edition.is_some()
            || self.hires_answers.contains_key(&track.album.id)
        {
            return;
        }
        let (track, format) = (track.clone(), format.clone());
        let (tidal, tx, request) = (self.tidal.clone(), self.tx.clone(), self.offer_req);
        let quality = self.quality.ceiling();
        tauri::async_runtime::spawn(async move {
            if let Some(offer) = find_offer(&tidal, &track, quality, &format).await {
                let _ = tx.send(Cmd::HiresOffer { request, offer }).await;
            }
        });
    }

    /// Answer to the banner, valid for the whole album during the session. Yes: the track is
    /// reloaded at the same position from the hi-res edition (tiny gap) and playback
    /// resumes; the preloaded next one, resolved without it, is removed.
    pub(super) async fn answer_hires_offer(&mut self, accept: bool) {
        let Some(album) = self.state.track.as_ref().map(|t| t.album.id) else {
            return;
        };
        if self.state.hires_offer.take().is_none() {
            return;
        }
        self.offer_req += 1;
        self.hires_answers.insert(album, accept);
        if accept && let Some(id) = self.queue.current() {
            self.invalidate_next().await;
            self.load_current(id, self.state.time_pos);
        }
    }
}

/// A failed lookup falls back to "no hi-res edition": the original is played.
fn lookup_result(track: &Track, result: crate::tidal::Result<Option<Track>>) -> Option<Track> {
    result.unwrap_or_else(|e| {
        tracing::warn!("recherche hi-res de {} impossible : {e}", track.id);
        None
    })
}

/// The other edition's stream is only kept if it is really hi-res and, if the original
/// stream is known, better on one axis without being worse on the other.
/// Also used by the track info (#3) to say whether the edition would be played.
pub fn is_better(candidate: &StreamFormat, original: Option<&StreamFormat>) -> bool {
    if candidate.quality != AudioQuality::HiResLossless {
        return false;
    }
    let Some(original) = original else {
        return true;
    };
    let depth = (
        candidate.bit_depth.unwrap_or(0),
        original.bit_depth.unwrap_or(0),
    );
    let rate = (
        candidate.sample_rate.unwrap_or(0),
        original.sample_rate.unwrap_or(0),
    );
    depth.0 >= depth.1 && rate.0 >= rate.1 && (depth.0 > depth.1 || rate.0 > rate.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(quality: AudioQuality, bits: u8, rate: u32) -> StreamFormat {
        StreamFormat {
            quality,
            codec: "flac".into(),
            bit_depth: Some(bits),
            sample_rate: Some(rate),
            bitrate: None,
            hires_edition: None,
        }
    }

    #[test]
    fn hires_beats_cd_quality() {
        let cd = format(AudioQuality::Lossless, 16, 44_100);
        let hires = format(AudioQuality::HiResLossless, 24, 96_000);
        assert!(is_better(&hires, Some(&cd)));
        assert!(is_better(&hires, None));
    }

    #[test]
    fn rejects_non_hires_or_worse_streams() {
        let cd = format(AudioQuality::Lossless, 16, 44_100);
        assert!(!is_better(&cd, None), "not hi-res");
        assert!(!is_better(&cd, Some(&cd)));
        let same = format(AudioQuality::HiResLossless, 16, 44_100);
        assert!(!is_better(&same, Some(&cd)), "not better");
        let lower_rate = format(AudioQuality::HiResLossless, 24, 44_100);
        let original = format(AudioQuality::Lossless, 16, 48_000);
        assert!(
            !is_better(&lower_rate, Some(&original)),
            "lower sample rate"
        );
    }

    /// `cargo test live_hires_playback -- --ignored --nocapture` : *Billie Jean* de
    /// "Thriller 25 Super Deluxe Edition" (16/44.1 only), resolved as the current track then
    /// decoded by a real mpv (`null` output).
    #[tokio::test]
    #[ignore = "network, TIDAL session and mpv required"]
    async fn live_hires_playback() {
        use serde_json::json;

        use crate::mpv::CacheSize;
        use crate::mpv::serve::ManifestServer;
        use crate::tidal::models::{StreamSource, TrackId};

        let tidal = TidalClient::new().unwrap();
        assert!(
            tidal.current_session().await.unwrap().is_some(),
            "no session"
        );
        let track = tidal.track(TrackId(1_632_061)).await.unwrap();
        for round in ["first try", "cached"] {
            let start = std::time::Instant::now();
            let stream = resolve_stream(&tidal, &track, AudioQuality::HiResLossless, true, true)
                .await
                .unwrap();
            println!("{round}: {:?} in {:?}", stream.format, start.elapsed());
            assert_eq!(stream.format.bit_depth, Some(24));
            assert!(stream.format.hires_edition.is_some());
        }
        let off = resolve_stream(&tidal, &track, AudioQuality::HiResLossless, false, true)
            .await
            .unwrap();
        assert_eq!(off.format.bit_depth, Some(16), "setting off");

        let stream = resolve_stream(&tidal, &track, AudioQuality::HiResLossless, true, false)
            .await
            .unwrap();
        let StreamSource::Dash(mpd) = stream.source else {
            panic!("DASH manifest expected");
        };
        let server = ManifestServer::start().await.unwrap();
        let url = server.publish(&track.id.to_string(), mpd);
        let conf = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("mpv/seaglass.conf");
        let mut child = crate::mpv::process::spawn(&conf, CacheSize::Mib32, None, false).unwrap();
        let (events_tx, mut events_rx) = tokio::sync::mpsc::channel(256);
        tokio::spawn(async move { while events_rx.recv().await.is_some() {} });
        let handle = crate::mpv::ipc::connect(events_tx).await.unwrap();
        handle
            .command(vec![json!("set_property"), json!("ao"), json!("null")])
            .await
            .unwrap();
        handle
            .command(vec![json!("loadfile"), json!(url)])
            .await
            .unwrap();
        let mut params = serde_json::Value::Null;
        for _ in 0..100 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            params = handle
                .command(vec![json!("get_property"), json!("audio-params")])
                .await
                .unwrap_or_default();
            if !params.is_null() {
                break;
            }
        }
        let codec = handle
            .command(vec![json!("get_property"), json!("audio-codec-name")])
            .await
            .unwrap();
        println!("mpv : {codec} {params}");
        let _ = child.kill().await;
        assert_eq!(codec, json!("flac"));
        assert_eq!(params["format"], json!("s32"));
        assert!(params["samplerate"].as_u64().unwrap() > 44_100);
    }
}
