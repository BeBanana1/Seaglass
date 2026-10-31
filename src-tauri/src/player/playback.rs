//! Fenêtre mpv de 2 entrées au maximum : titre courant (0) + suivant préchargé (1).
//!
//! Les URLs de flux sont signées et expirent : on ne charge jamais plus loin.

use serde_json::{Value, json};

use super::actor::{Cmd, Player, Slot};
use super::normalize::file_options;
use super::queue::Repeat;
use super::settings::HiresUpgrade;
use super::upgrade;
use crate::i18n::{tr, trf};
use crate::mpv::{CacheSize, EndReason};
use crate::tidal::models::{AudioQuality, Stream, StreamSource, Track, TrackId};

/// Qualité demandée ; deviendra un réglage utilisateur.
pub(super) const QUALITY: AudioQuality = AudioQuality::HiResLossless;
/// Préchargement du suivant quand il reste moins que ça sur le titre courant.
const PRELOAD_BEFORE_END: f64 = 30.0;
/// En deçà, « Précédent » revient au titre précédent plutôt qu'au début du titre.
const RESTART_THRESHOLD: f64 = 3.0;
/// Échecs consécutifs au-delà desquels on arrête d'enchaîner.
const MAX_FAILURES: u32 = 3;

impl Player {
    /// Charge `id` en entrée 0 (remplace toute la fenêtre mpv) à partir de `start`.
    pub(super) fn load_current(&mut self, id: TrackId, start: f64) {
        if let Err(e) = &self.mpv {
            let message = e.clone();
            self.toast("error", message);
            return;
        }
        if self.retried != Some(id) {
            self.retried = None;
        }
        self.load_req += 1;
        self.preload_req += 1;
        self.preloading = false;
        self.next_loaded = None;
        self.loaded = false;
        self.state.loading = true;
        self.state.track = self.tidal.cache.get(id);
        self.state.stream = None;
        self.state.hires_offer = None;
        self.state.normalization = None;
        self.offer_req += 1;
        self.state.time_pos = start;
        self.resolve(id, Slot::Current { start }, self.load_req);
    }

    /// Précharge le titre suivant à moins de 30 s de la fin (sauf repeat One : `loop-file`).
    pub(super) fn maybe_preload(&mut self) {
        let remaining = self.state.duration - self.state.time_pos;
        if self.next_loaded.is_some()
            || self.preloading
            || self.state.duration <= 0.0
            || remaining >= PRELOAD_BEFORE_END
            || self.queue.repeat() == Repeat::One
        {
            return;
        }
        let Some(id) = self.queue.peek_next() else {
            return;
        };
        self.preload_req += 1;
        self.preloading = true;
        self.resolve(id, Slot::Next(id), self.preload_req);
    }

    /// Retire le titre préchargé de mpv et annule un préchargement en cours.
    pub(super) async fn invalidate_next(&mut self) {
        self.preload_req += 1;
        self.preloading = false;
        if self.next_loaded.take().is_some() {
            self.mpv_command(vec![json!("playlist-remove"), json!(1)])
                .await;
        }
    }

    /// Résout métadonnées et flux hors de l'acteur, qui reste réactif pendant ce temps.
    /// Le flux peut venir d'une autre édition hi-res (`upgrade::resolve_stream`) : la file
    /// et l'affichage gardent `id`. En mode « Demander », seulement pour un album dont
    /// l'offre a été acceptée.
    fn resolve(&self, id: TrackId, slot: Slot, request: u64) {
        let (tidal, tx) = (self.tidal.clone(), self.tx.clone());
        let mode = self.hires_upgrade;
        let accepted: Vec<u64> = self
            .hires_answers
            .iter()
            .filter_map(|(album, yes)| yes.then_some(*album))
            .collect();
        let current = matches!(slot, Slot::Current { .. });
        tauri::async_runtime::spawn(async move {
            let result = async {
                let track = tidal.track(id).await?;
                let upgrade = match mode {
                    HiresUpgrade::Auto => true,
                    HiresUpgrade::Ask => accepted.contains(&track.album.id),
                    HiresUpgrade::Off => false,
                };
                let stream =
                    upgrade::resolve_stream(&tidal, &track, QUALITY, upgrade, current).await?;
                Ok::<_, crate::tidal::TidalError>(Box::new((track, stream)))
            }
            .await
            .map_err(|e| e.to_string());
            let _ = tx
                .send(Cmd::Resolved {
                    request,
                    slot,
                    result,
                })
                .await;
        });
    }

    pub(super) async fn on_resolved(
        &mut self,
        request: u64,
        slot: Slot,
        result: Result<Box<(Track, Stream)>, String>,
    ) {
        match slot {
            Slot::Current { start } if request == self.load_req => {
                self.state.loading = false;
                let (track, stream) = match result {
                    Ok(resolved) => *resolved,
                    Err(e) => return self.on_track_failed(e).await,
                };
                let Some(url) = self.stream_url(&track, &stream) else {
                    return;
                };
                log_stream("lecture", &track, &stream);
                // Gain de normalisation en option du fichier (#82) : il ne touche pas le
                // titre qui sera préchargé ensuite.
                let gain = self.normalizer.loaded(stream.gain);
                let mut cmd = vec![json!("loadfile"), json!(url), json!("replace")];
                if let Some(options) = file_options(start, gain) {
                    cmd.extend([json!(-1), json!(options)]);
                }
                self.state.normalization = gain;
                self.mpv_command(cmd).await;
                self.mpv_command(vec![json!("set_property"), json!("pause"), json!(false)])
                    .await;
                self.loaded = true;
                // mpv n'émet rien si `pause` valait déjà `false` : l'état est fixé ici.
                self.state.paused = false;
                self.state.track = Some(track);
                self.state.stream = Some(stream.format);
                self.maybe_offer_hires();
            }
            Slot::Next(id) if request == self.preload_req => {
                self.preloading = false;
                match result {
                    Ok(resolved) => {
                        let (track, stream) = *resolved;
                        let Some(url) = self.stream_url(&track, &stream) else {
                            return;
                        };
                        log_stream("préchargement", &track, &stream);
                        let gain = self.normalizer.preloaded(stream.gain);
                        let mut cmd = vec![json!("loadfile"), json!(url), json!("append")];
                        if let Some(options) = file_options(0.0, gain) {
                            cmd.extend([json!(-1), json!(options)]);
                        }
                        self.mpv_command(cmd).await;
                        self.next_loaded = Some((track, stream.format));
                    }
                    // Sans préchargement, la fin du titre déclenchera un chargement classique.
                    Err(e) => tracing::warn!("préchargement de {id} impossible : {e}"),
                }
            }
            _ => {} // Résolution remplacée par une plus récente.
        }
    }

    /// mpv est passé sur l'entrée préchargée : on avance la file et on retire l'ancienne.
    pub(super) async fn on_playlist_pos(&mut self, pos: Option<i64>) {
        if pos != Some(1) {
            return;
        }
        let Some((track, format)) = self.next_loaded.take() else {
            return;
        };
        let advanced = self.queue.advance();
        self.mark_queue_dirty();
        self.mpv_command(vec![json!("playlist-remove"), json!(0)])
            .await;
        if advanced == Some(track.id) {
            self.retried = None;
            self.state.track = Some(track);
            self.state.stream = Some(format);
            self.state.normalization = self.normalizer.advance();
            self.state.time_pos = 0.0;
            self.maybe_offer_hires();
        } else if let Some(id) = advanced {
            // Ne devrait pas arriver : toute modification de la file retire l'entrée 1.
            tracing::warn!("préchargé {} mais la file donne {id}", track.id);
            self.load_current(id, 0.0);
        }
    }

    pub(super) async fn on_end_file(&mut self, reason: EndReason, error: Option<String>) {
        match reason {
            // Fin naturelle sans suivant préchargé (préchargement absent ou en échec).
            EndReason::Eof if self.loaded && self.next_loaded.is_none() => {
                self.loaded = false;
                if let Some(id) = self.queue.advance() {
                    self.mark_queue_dirty();
                    self.load_current(id, 0.0);
                }
            }
            EndReason::Error => self.on_playback_error(error).await,
            _ => {}
        }
    }

    /// Erreur mpv : une nouvelle URL une fois (signature expirée ?), puis titre suivant.
    async fn on_playback_error(&mut self, error: Option<String>) {
        let Some(id) = self.queue.current() else {
            return;
        };
        let detail = error.unwrap_or_else(|| tr("erreur inconnue", "unknown error").into());
        if self.retried != Some(id) {
            tracing::warn!("erreur de lecture sur {id} ({detail}), nouvelle URL");
            self.retried = Some(id);
            self.load_current(id, self.state.time_pos);
        } else {
            self.on_track_failed(trf!(
                "lecture impossible : {detail}",
                "cannot play: {detail}"
            ))
            .await;
        }
    }

    /// Le titre courant ne peut pas être lu : toast, puis titre suivant.
    async fn on_track_failed(&mut self, message: String) {
        self.toast("error", message);
        self.failures += 1;
        self.loaded = false;
        if self.failures >= MAX_FAILURES {
            self.failures = 0;
            self.toast(
                "error",
                tr(
                    "trop d'échecs consécutifs, lecture arrêtée",
                    "too many consecutive failures, playback stopped",
                )
                .into(),
            );
            self.mpv_command(vec![json!("stop")]).await;
            return;
        }
        if let Some(id) = self.queue.advance() {
            self.mark_queue_dirty();
            self.load_current(id, 0.0);
        }
    }

    pub(super) async fn skip_next(&mut self) {
        if let Some(id) = self.queue.advance() {
            self.on_queue_changed().await;
            self.load_current(id, 0.0);
        }
    }

    /// Au-delà de 3 s : retour au début du titre ; sinon titre précédent de l'historique.
    pub(super) async fn previous(&mut self) {
        if self.state.time_pos > RESTART_THRESHOLD || !self.queue.has_previous() {
            return self.seek(0.0).await;
        }
        if let Some(id) = self.queue.previous() {
            self.on_queue_changed().await;
            self.load_current(id, 0.0);
        }
    }

    /// Après une restauration, le titre n'est pas encore dans mpv : on le charge à la
    /// position sauvegardée.
    pub(super) async fn toggle_pause(&mut self) {
        if self.loaded {
            self.mpv_command(vec![json!("cycle"), json!("pause")]).await;
        } else if let Some(id) = self.queue.current() {
            self.load_current(id, self.state.time_pos);
        }
    }

    /// Touches média : Lecture relance un titre restauré, Pause est sans effet s'il n'y a
    /// rien de chargé.
    pub(super) async fn set_paused(&mut self, paused: bool) {
        if self.loaded {
            self.mpv_command(vec![json!("set_property"), json!("pause"), json!(paused)])
                .await;
        } else if !paused && let Some(id) = self.queue.current() {
            self.load_current(id, self.state.time_pos);
        }
    }

    pub(super) async fn seek(&mut self, secs: f64) {
        if self.loaded {
            self.mpv_command(vec![json!("seek"), json!(secs), json!("absolute")])
                .await;
        } else {
            self.state.time_pos = secs;
            self.persist_position = true;
        }
    }

    /// Repeat One = `loop-file=inf` côté mpv, sans rechargement.
    pub(super) async fn apply_loop_file(&mut self) {
        if self.mpv.is_err() {
            return;
        }
        let value = if self.queue.repeat() == Repeat::One {
            "inf"
        } else {
            "no"
        };
        self.mpv_command(vec![
            json!("set_property"),
            json!("loop-file"),
            json!(value),
        ])
        .await;
    }

    pub(super) async fn on_volume_changed(&mut self) {
        self.persist_volume = true;
        self.apply_volume().await;
    }

    /// Volume logiciel de mpv (`volume`, courbe cubique, 100 = gain unitaire) et coupure.
    pub(super) async fn apply_volume(&mut self) {
        if self.mpv.is_err() {
            return;
        }
        let v = self.state.volume;
        self.mpv_command(vec![json!("set_property"), json!("volume"), json!(v.level)])
            .await;
        self.mpv_command(vec![json!("set_property"), json!("mute"), json!(v.muted)])
            .await;
    }

    /// Nouvelle taille de cache : mpv l'applique aussi au démuxeur déjà ouvert, sans
    /// rechargement, dès que la lecture consomme des paquets (vérifié par le test
    /// `mpv::cache::tests::live_cache_resize`). Sans mpv, elle servira au prochain
    /// lancement (lue dans les réglages).
    pub(super) async fn apply_cache_size(&mut self, size: CacheSize) {
        if self.mpv.is_err() {
            return;
        }
        for command in size.set_property_commands() {
            self.mpv_command(command).await;
        }
    }

    fn stream_url(&self, track: &Track, stream: &Stream) -> Option<String> {
        match (&stream.source, &self.server) {
            (StreamSource::Direct(url), _) => Some(url.clone()),
            (StreamSource::Dash(mpd), Some(server)) => {
                Some(server.publish(&track.id.to_string(), mpd.clone()))
            }
            (StreamSource::Dash(_), None) => {
                self.toast(
                    "error",
                    tr(
                        "serveur de manifestes indisponible",
                        "manifest server unavailable",
                    )
                    .into(),
                );
                None
            }
        }
    }

    pub(super) async fn mpv_command(&mut self, command: Vec<Value>) {
        let result = match &self.mpv {
            Ok(mpv) => mpv
                .handle
                .command(command)
                .await
                .map(drop)
                .map_err(|e| e.to_string()),
            Err(e) => Err(e.clone()),
        };
        if let Err(e) = result {
            self.toast("error", e);
        }
    }
}

fn log_stream(what: &str, track: &Track, stream: &Stream) {
    let f = &stream.format;
    tracing::info!(
        "{what} {} : {:?} {} {:?}/{:?}",
        track.id,
        f.quality,
        f.codec,
        f.bit_depth,
        f.sample_rate
    );
}
