//! Choix du flux d'un titre : le sien, ou celui d'une autre édition hi-res du même
//! enregistrement (#18). Tourne hors de l'acteur, dans la résolution réseau.
//!
//! - Titre suivant (préchargement à 30 s de la fin) : recherche puis flux, l'un après
//!   l'autre ; le temps passé est invisible.
//! - Titre courant : la recherche part en parallèle du flux d'origine et ne peut retarder
//!   le démarrage de plus de `CURRENT_LOOKUP_LIMIT`. Une réponse déjà en cache évite la
//!   demande du flux d'origine.
//! - Mode « Demander » (#59) : le titre d'origine démarre sans attendre, `find_offer`
//!   cherche ensuite l'édition à proposer.

use std::time::Duration;

use super::actor::{Cmd, Player};
use super::settings::HiresUpgrade;
use crate::tidal::TidalClient;
use crate::tidal::models::{AudioQuality, Stream, StreamFormat, Track};

/// Temps maximal accordé à la recherche d'une édition hi-res pour le titre courant.
const CURRENT_LOOKUP_LIMIT: Duration = Duration::from_millis(1500);

/// Flux à lire pour `track`. `current` : le titre est attendu tout de suite (entrée 0).
pub(super) async fn resolve_stream(
    tidal: &TidalClient,
    track: &Track,
    quality: AudioQuality,
    upgrade: bool,
    current: bool,
) -> crate::tidal::Result<Stream> {
    if !upgrade || track.hires || quality != AudioQuality::HiResLossless {
        return tidal.stream(track.id, quality).await;
    }
    let (original, edition) = match tidal.known_hires_edition(track.id) {
        Some(known) => (None, known),
        None if current => {
            let lookup = tokio::time::timeout(CURRENT_LOOKUP_LIMIT, tidal.hires_edition(track));
            let (original, edition) = tokio::join!(tidal.stream(track.id, quality), lookup);
            let edition = match edition {
                Ok(found) => found,
                Err(_) => {
                    tracing::warn!("recherche hi-res de {} trop lente, ignorée", track.id);
                    Ok(None)
                }
            };
            (Some(original), lookup_result(track, edition))
        }
        None => (None, lookup_result(track, tidal.hires_edition(track).await)),
    };
    if let Some(edition) = edition {
        let base = original.as_ref().and_then(|r| r.as_ref().ok());
        match tidal.stream(edition.id, quality).await {
            Ok(mut stream) if is_better(&stream.format, base.map(|s| &s.format)) => {
                tracing::info!(
                    "{} : édition hi-res {} ({})",
                    track.id,
                    edition.id,
                    edition.album.title
                );
                stream.format.hires_edition = Some((&edition).into());
                return Ok(stream);
            }
            Ok(stream) => tracing::info!(
                "{} : édition {} pas meilleure ({:?}/{:?}), version d'origine",
                track.id,
                edition.id,
                stream.format.bit_depth,
                stream.format.sample_rate
            ),
            Err(e) => tracing::warn!("{} : flux de l'édition {} : {e}", track.id, edition.id),
        }
    }
    match original {
        Some(result) => result,
        None => tidal.stream(track.id, quality).await,
    }
}

/// Édition hi-res à proposer pour `track`, joué au format `original` : son format (avec
/// l'album dans `hires_edition`) si elle est réellement meilleure. Rien n'est lu : le flux
/// sera demandé si l'offre est acceptée.
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
            tracing::warn!("{} : format de l'édition {} : {e}", track.id, edition.id);
            None
        }
    }
}

impl Player {
    /// Mode « Demander » (#59) : cherche en arrière-plan l'édition hi-res à proposer pour
    /// le titre courant, sauf si son album a déjà eu une réponse. Retire toute offre
    /// précédente (autre titre, autre réglage).
    pub(super) fn maybe_offer_hires(&mut self) {
        self.offer_req += 1;
        self.state.hires_offer = None;
        // Pas pendant un blind test (#434) : le bandeau nommerait l'album.
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

    /// Réponse au bandeau, valable pour tout l'album pendant la session. Oui : le titre est
    /// rechargé à la même position depuis l'édition hi-res (micro-coupure) et la lecture
    /// reprend ; le suivant préchargé, résolu sans elle, est retiré.
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

/// Une recherche en échec revient à « pas d'édition hi-res » : on lit l'original.
fn lookup_result(track: &Track, result: crate::tidal::Result<Option<Track>>) -> Option<Track> {
    result.unwrap_or_else(|e| {
        tracing::warn!("recherche hi-res de {} impossible : {e}", track.id);
        None
    })
}

/// Le flux de l'autre édition n'est retenu que s'il est réellement hi-res et, si le flux
/// d'origine est connu, meilleur sur un axe sans être moins bon sur l'autre.
/// Aussi utilisé par les infos du titre (#3) pour dire si l'édition serait lue.
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
        assert!(!is_better(&cd, None), "pas hi-res");
        assert!(!is_better(&cd, Some(&cd)));
        let same = format(AudioQuality::HiResLossless, 16, 44_100);
        assert!(!is_better(&same, Some(&cd)), "pas meilleur");
        let lower_rate = format(AudioQuality::HiResLossless, 24, 44_100);
        let original = format(AudioQuality::Lossless, 16, 48_000);
        assert!(
            !is_better(&lower_rate, Some(&original)),
            "fréquence plus basse"
        );
    }

    /// `cargo test live_hires_playback -- --ignored --nocapture` : *Billie Jean* de
    /// « Thriller 25 Super Deluxe Edition » (16/44,1 seulement), résolu comme titre courant
    /// puis décodé par un vrai mpv (sortie `null`).
    #[tokio::test]
    #[ignore = "réseau, session TIDAL et mpv requis"]
    async fn live_hires_playback() {
        use serde_json::json;

        use crate::mpv::CacheSize;
        use crate::mpv::serve::ManifestServer;
        use crate::tidal::models::{StreamSource, TrackId};

        let tidal = TidalClient::new().unwrap();
        assert!(
            tidal.current_session().await.unwrap().is_some(),
            "pas de session"
        );
        let track = tidal.track(TrackId(1_632_061)).await.unwrap();
        for round in ["premier essai", "en cache"] {
            let start = std::time::Instant::now();
            let stream = resolve_stream(&tidal, &track, AudioQuality::HiResLossless, true, true)
                .await
                .unwrap();
            println!("{round} : {:?} en {:?}", stream.format, start.elapsed());
            assert_eq!(stream.format.bit_depth, Some(24));
            assert!(stream.format.hires_edition.is_some());
        }
        let off = resolve_stream(&tidal, &track, AudioQuality::HiResLossless, false, true)
            .await
            .unwrap();
        assert_eq!(off.format.bit_depth, Some(16), "réglage désactivé");

        let stream = resolve_stream(&tidal, &track, AudioQuality::HiResLossless, true, false)
            .await
            .unwrap();
        let StreamSource::Dash(mpd) = stream.source else {
            panic!("manifeste DASH attendu");
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
