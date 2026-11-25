//! Fenêtre du moteur de 2 entrées au maximum : titre courant (0) + suivant préchargé (1).
//!
//! Les URLs de flux sont signées et expirent : on ne charge jamais plus loin.

use super::actor::{Cmd, Player, Slot};
use super::queue::Repeat;
use super::settings::HiresUpgrade;
use super::upgrade;
use crate::engine::{End, Engine, Op};
use crate::i18n::{tr, trf};
use crate::mpv::CacheSize;
use crate::tidal::models::{AudioQuality, Stream, StreamSource, StreamUse, Track, TrackId};

/// Préchargement du suivant quand il reste moins que ça sur le titre courant.
const PRELOAD_BEFORE_END: f64 = 30.0;
/// En deçà, « Précédent » revient au titre précédent plutôt qu'au début du titre.
const RESTART_THRESHOLD: f64 = 3.0;
/// Échecs consécutifs au-delà desquels on arrête d'enchaîner.
const MAX_FAILURES: u32 = 3;

impl Player {
    /// Charge `id` en entrée 0 (remplace toute la fenêtre du moteur) à partir de `start`.
    pub(super) fn load_current(&mut self, id: TrackId, start: f64) {
        if let Err(e) = &self.engine {
            let message = e.clone();
            self.toast("error", message);
            return;
        }
        if self.retried != Some(id) {
            self.retried = None;
        }
        self.log_close();
        self.load_req += 1;
        self.preload_req += 1;
        self.preloading = false;
        self.next_loaded = None;
        self.loaded = false;
        self.state.loading = true;
        self.state.ended = false;
        self.state.track = self.tidal.cache.get(id);
        self.state.stream = None;
        self.state.hires_offer = None;
        self.state.normalization = None;
        self.offer_req += 1;
        self.state.time_pos = start;
        self.state.cached = 0.0;
        self.resolve(id, Slot::Current { start }, self.load_req);
    }

    /// Précharge le titre suivant à moins de 30 s de la fin (sauf repeat One : `loop-file`,
    /// et minuteur « Fin du titre » : rien ne doit suivre en gapless ; blind test : chaque
    /// manche est chargée seule, #434).
    pub(super) fn maybe_preload(&mut self) {
        let remaining = self.state.duration - self.state.time_pos;
        if self.next_loaded.is_some()
            || self.blind.is_some()
            || self.preloading
            || self.state.duration <= 0.0
            || remaining >= PRELOAD_BEFORE_END
            || self.queue.repeat() == Repeat::One
            || self.sleeps_at_end_of_track()
        {
            return;
        }
        let Some(id) = self.queue.peek_next() else {
            // Fin de file : la radio, si elle est activée, sera préchargée à son arrivée.
            return self.maybe_request_radio();
        };
        self.preload_req += 1;
        self.preloading = true;
        self.resolve(id, Slot::Next(id), self.preload_req);
    }

    /// Retire le titre préchargé du moteur et annule un préchargement en cours.
    pub(super) async fn invalidate_next(&mut self) {
        self.preload_req += 1;
        self.preloading = false;
        if self.next_loaded.take().is_some() {
            self.engine_op(Op::RemoveNext).await;
        }
    }

    /// Résout métadonnées et flux hors de l'acteur, qui reste réactif pendant ce temps.
    /// Le flux peut venir d'une autre édition hi-res (`upgrade::resolve_stream`) : la file
    /// et l'affichage gardent `id`. En mode « Demander », seulement pour un album dont
    /// l'offre a été acceptée.
    fn resolve(&self, id: TrackId, slot: Slot, request: u64) {
        let (tidal, tx) = (self.tidal.clone(), self.tx.clone());
        let (mode, policy) = (self.hires_upgrade, self.quality);
        let accepted: Vec<u64> = self
            .hires_answers
            .iter()
            .filter_map(|(album, yes)| yes.then_some(*album))
            .collect();
        let current = matches!(slot, Slot::Current { .. });
        tauri::async_runtime::spawn(async move {
            let result = async {
                // Sur Android, lit le réseau et la batterie de l'instant (#365).
                let quality = policy.current().await;
                // Sans édition hi-res possible (Android, réglage « Non »), le flux ne dépend
                // pas des métadonnées : les deux requêtes partent ensemble (#148).
                if quality != AudioQuality::HiResLossless || mode == HiresUpgrade::Off {
                    let use_ = if current {
                        StreamUse::Play
                    } else {
                        StreamUse::Preload
                    };
                    let (track, stream) =
                        tokio::join!(tidal.track(id), tidal.stream(id, quality, use_));
                    return Ok(Box::new((track?, stream?)));
                }
                let track = tidal.track(id).await?;
                let upgrade = match mode {
                    HiresUpgrade::Auto => true,
                    HiresUpgrade::Ask => accepted.contains(&track.album.id),
                    HiresUpgrade::Off => false,
                };
                let stream =
                    upgrade::resolve_stream(&tidal, &track, quality, upgrade, current).await?;
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
                self.state.normalization = gain;
                let filters = self.filters(gain);
                self.engine_op(Op::Play {
                    url,
                    start,
                    filters,
                })
                .await;
                self.loaded = true;
                self.log_loaded(stream.session, track.album.id, start);
                // The engine emits nothing if it was not paused: the state is set here.
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
                        let filters = self.filters(gain);
                        self.engine_op(Op::Preload { url, filters }).await;
                        self.next_loaded = Some((track, stream.format, stream.session));
                    }
                    // Sans préchargement, la fin du titre déclenchera un chargement classique.
                    Err(e) => tracing::warn!("préchargement de {id} impossible : {e}"),
                }
            }
            _ => {} // Résolution remplacée par une plus récente.
        }
    }

    /// Le moteur est passé sur l'entrée préchargée : on avance la file et on retire
    /// l'ancienne.
    pub(super) async fn on_advanced(&mut self) {
        let Some((track, format, session)) = self.next_loaded.take() else {
            return;
        };
        let advanced = self.queue.advance();
        self.mark_queue_dirty();
        self.engine_op(Op::DropPrevious).await;
        if advanced == Some(track.id) {
            // The engine may already report the next track's duration: the previous one ends
            // at its own.
            let previous = self
                .state
                .track
                .as_ref()
                .map_or(self.state.duration, |t| f64::from(t.duration));
            self.log_advanced(session, track.album.id, previous);
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

    pub(super) async fn on_end_file(&mut self, end: End) {
        match end {
            // Fin naturelle sans suivant préchargé (préchargement absent ou en échec).
            End::Eof if self.loaded && self.next_loaded.is_none() => {
                self.log_ended();
                self.loaded = false;
                if self.sleeps_at_end_of_track() {
                    self.sleep_after_track().await;
                } else if let Some(id) = self.queue.advance() {
                    self.mark_queue_dirty();
                    self.load_current(id, 0.0);
                } else {
                    self.on_queue_end();
                }
            }
            End::Eof => {}
            // Périphérique de sortie choisi qui ne s'ouvre pas : repli, pas une erreur du flux.
            End::OutputFailed => {
                // Exclusif : pas de repli silencieux en partagé ni sur un autre périphérique.
                if self.state.exclusive {
                    self.on_exclusive_failed().await;
                } else if !self.on_output_init_failed().await {
                    let detail = tr(
                        "ouverture de la sortie audio impossible",
                        "audio output initialization failed",
                    );
                    self.on_playback_error(Some(detail.into())).await;
                }
            }
            End::Error(error) => self.on_playback_error(error).await,
        }
    }

    /// Erreur du moteur : une nouvelle URL une fois (signature expirée ?), puis titre
    /// suivant.
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
        self.log_close();
        self.toast_kind("error", Some("unplayable"), message);
        self.failures += 1;
        self.loaded = false;
        if self.failures >= MAX_FAILURES {
            // Blind test (#434) : la partie s'arrête aussi, la file d'avant revient.
            self.end_blind_test().await;
            self.failures = 0;
            self.toast_kind(
                "error",
                Some("stopped"),
                tr(
                    "trop d'échecs consécutifs, lecture arrêtée",
                    "too many consecutive failures, playback stopped",
                )
                .into(),
            );
            self.engine_op(Op::Stop).await;
            return;
        }
        if self.blind_track_failed().await {
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

    /// Après une restauration, le titre n'est pas encore dans le moteur : on le charge à la
    /// position sauvegardée.
    pub(super) async fn toggle_pause(&mut self) {
        if self.loaded {
            self.engine_op(Op::TogglePause).await;
        } else if let Some(id) = self.queue.current() {
            self.load_current(id, self.state.time_pos);
        }
    }

    /// Touches média : Lecture relance un titre restauré, Pause est sans effet s'il n'y a
    /// rien de chargé.
    pub(super) async fn set_paused(&mut self, paused: bool) {
        if self.loaded {
            self.engine_op(Op::SetPaused(paused)).await;
        } else if !paused && let Some(id) = self.queue.current() {
            self.load_current(id, self.state.time_pos);
        }
    }

    pub(super) async fn seek(&mut self, secs: f64) {
        if self.loaded {
            self.log_seek(secs);
            self.engine_op(Op::Seek(secs)).await;
        } else {
            self.state.time_pos = secs;
            self.persist_position = true;
        }
    }

    /// Repeat One = boucle du titre dans le moteur (`loop-file=inf` sous mpv), sans
    /// rechargement ; levée tant que le minuteur vise la fin du titre (#85).
    pub(super) async fn apply_loop_file(&mut self) {
        if self.engine.is_err() {
            return;
        }
        let on = self.queue.repeat() == Repeat::One && !self.sleeps_at_end_of_track();
        self.engine_op(Op::LoopCurrent(on)).await;
    }

    pub(super) async fn on_volume_changed(&mut self) {
        self.persist_volume = true;
        self.note_device_profile();
        self.apply_volume().await;
    }

    /// Volume logiciel du moteur (mpv : courbe cubique, 100 = gain unitaire) et coupure.
    pub(super) async fn apply_volume(&mut self) {
        if self.engine.is_err() {
            return;
        }
        let v = self.state.volume;
        self.engine_op(Op::Volume {
            level: v.level,
            muted: v.muted,
        })
        .await;
    }

    /// Nouvelle taille de cache : mpv l'applique aussi au démuxeur déjà ouvert, sans
    /// rechargement, dès que la lecture consomme des paquets (vérifié par le test
    /// `mpv::cache::tests::live_cache_resize`). Sans mpv, elle servira au prochain
    /// lancement (lue dans les réglages).
    pub(super) async fn apply_cache_size(&mut self, size: CacheSize) {
        if self.engine.is_err() {
            return;
        }
        self.engine_op(Op::CacheSize(size)).await;
    }

    fn stream_url(&self, track: &Track, stream: &Stream) -> Option<String> {
        // Android : le MPD part en URL `data:`, sans serveur local.
        #[cfg(mobile)]
        if let StreamSource::Dash(mpd) = &stream.source {
            return Some(crate::media3::data_url(mpd));
        }
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

    /// Envoie une opération au moteur ; une erreur (moteur absent compris) part en toast.
    pub(super) async fn engine_op(&mut self, op: Op) {
        let result = match &self.engine {
            Ok(engine) => engine.apply(op).await,
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
