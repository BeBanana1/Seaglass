//! Normalisation du volume (#82) : gain ReplayGain de l'API (pas les tags du FLAC),
//! limité par la crête pour ne jamais écrêter, appliqué par le moteur **à chaque fichier**
//! séparément (`Op::Play`, `Op::Preload`, `Op::CurrentGain`).
//!
//! Sous mpv (`mpv::engine`), vérifié avec mpv 0.41 (`live_per_file_af`) : `loadfile <url> append -1 af=…` pose le
//! filtre en option locale du fichier préchargé ; le titre en cours garde le sien jusqu'au
//! passage gapless, et un fichier chargé sans option revient à `af=[]`. Pour le titre en
//! cours, `file-local-options/af` change le filtre à chaud ; la valeur d'avant est remise
//! à la fin du fichier (`options/af` montre la valeur en vigueur, pas celle-là).

use serde::{Deserialize, Serialize};

use super::actor::Player;
use crate::engine::Op;
use crate::tidal::models::ReplayGain;

/// Bornes du gain appliqué, en dB : au-delà, une valeur de l'API serait suspecte.
const MIN_GAIN: f64 = -24.0;
const MAX_GAIN: f64 = 12.0;

/// Réglage Réglages › Lecture › Normalisation du volume.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Normalization {
    /// Signal intact (seul mode compatible avec une sortie bit-perfect).
    #[default]
    Off,
    /// Chaque titre au même volume perçu.
    Track,
    /// Écarts d'un album conservés, albums ramenés au même volume.
    Album,
}

/// Gain à appliquer en dB, ou `None` (réglage désactivé, gain inconnu, ou nul après
/// arrondi : le signal n'est alors pas modifié). Mode album sans gain d'album : gain du
/// titre. La crête limite le gain : `crête × 10^(gain/20) ≤ 1`.
pub fn gain_db(mode: Normalization, rg: &ReplayGain) -> Option<f64> {
    let (gain, peak) = match mode {
        Normalization::Off => return None,
        Normalization::Album if rg.album_gain.is_some() => (rg.album_gain?, rg.album_peak),
        Normalization::Track | Normalization::Album => (rg.track_gain?, rg.track_peak),
    };
    if !gain.is_finite() {
        return None;
    }
    let ceiling = peak
        .filter(|p| p.is_finite() && *p > 0.0)
        .map_or(MAX_GAIN, |p| -20.0 * p.log10());
    let gain = gain.min(ceiling).clamp(MIN_GAIN, MAX_GAIN);
    let rounded = (gain * 100.0).round() / 100.0;
    (rounded != 0.0).then_some(rounded)
}

/// État de la normalisation tenu par l'acteur : ReplayGain du titre courant et du suivant
/// préchargé, pour recalculer le gain quand le réglage change.
#[derive(Debug, Clone, Default)]
pub(super) struct Normalizer {
    pub(super) mode: Normalization,
    current: ReplayGain,
    next: ReplayGain,
}

impl Normalizer {
    pub(super) fn new(mode: Normalization) -> Self {
        Self {
            mode,
            ..Self::default()
        }
    }

    /// Titre chargé en entrée 0 : gain à lui appliquer.
    pub(super) fn loaded(&mut self, rg: ReplayGain) -> Option<f64> {
        self.current = rg;
        gain_db(self.mode, &rg)
    }

    /// Titre préchargé en entrée 1 : gain posé avec son `loadfile`.
    pub(super) fn preloaded(&mut self, rg: ReplayGain) -> Option<f64> {
        self.next = rg;
        gain_db(self.mode, &rg)
    }

    /// Passage gapless sur le préchargé : son gain devient le courant.
    pub(super) fn advance(&mut self) -> Option<f64> {
        self.current = std::mem::take(&mut self.next);
        gain_db(self.mode, &self.current)
    }

    pub(super) fn current_gain(&self) -> Option<f64> {
        gain_db(self.mode, &self.current)
    }
}

impl Player {
    /// Nouveau réglage : appliqué tout de suite au titre en cours, et le suivant préchargé
    /// est retiré pour être rechargé avec son gain.
    pub(super) async fn set_normalization(&mut self, mode: Normalization) {
        if mode == self.normalizer.mode {
            return;
        }
        self.normalizer.mode = mode;
        let gain = self.normalizer.current_gain();
        if self.loaded {
            self.engine_op(Op::CurrentGain(gain)).await;
            self.state.normalization = gain;
        }
        self.invalidate_next().await;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn rg(track: (f64, f64), album: Option<(f64, f64)>) -> ReplayGain {
        ReplayGain {
            track_gain: Some(track.0),
            track_peak: Some(track.1),
            album_gain: album.map(|a| a.0),
            album_peak: album.map(|a| a.1),
        }
    }

    #[test]
    fn gain_follows_mode_and_falls_back_to_track() {
        let r = rg((-6.5, 0.9), Some((-4.0, 0.99)));
        assert_eq!(gain_db(Normalization::Off, &r), None);
        assert_eq!(gain_db(Normalization::Track, &r), Some(-6.5));
        assert_eq!(gain_db(Normalization::Album, &r), Some(-4.0));
        let no_album = rg((-6.5, 0.9), None);
        assert_eq!(gain_db(Normalization::Album, &no_album), Some(-6.5));
        assert_eq!(gain_db(Normalization::Track, &ReplayGain::default()), None);
    }

    #[test]
    fn peak_prevents_clipping() {
        // +6 dB demandés, crête à 0,7 : au plus −20·log10(0,7) ≈ +3,10 dB.
        let g = gain_db(Normalization::Track, &rg((6.0, 0.7), None)).unwrap();
        assert!((g - 3.1).abs() < 0.01, "{g}");
        assert!(0.7 * 10f64.powf(g / 20.0) <= 1.0 + 1e-3);
        // Crête pleine échelle : aucun gain positif, une atténuation passe telle quelle.
        assert_eq!(gain_db(Normalization::Track, &rg((2.0, 1.0), None)), None);
        assert_eq!(
            gain_db(Normalization::Track, &rg((-3.0, 1.0), None)),
            Some(-3.0)
        );
        // Crête inconnue ou absurde : seulement les bornes.
        let no_peak = ReplayGain {
            track_gain: Some(20.0),
            ..ReplayGain::default()
        };
        assert_eq!(gain_db(Normalization::Track, &no_peak), Some(MAX_GAIN));
        assert_eq!(
            gain_db(Normalization::Track, &rg((-40.0, 0.0), None)),
            Some(MIN_GAIN)
        );
        assert_eq!(
            gain_db(Normalization::Track, &rg((f64::NAN, 0.5), None)),
            None
        );
    }

    #[test]
    fn normalizer_tracks_current_and_next() {
        let mut n = Normalizer::new(Normalization::Track);
        assert_eq!(n.loaded(rg((-5.0, 0.5), None)), Some(-5.0));
        assert_eq!(n.preloaded(rg((-2.0, 0.5), None)), Some(-2.0));
        assert_eq!(n.current_gain(), Some(-5.0));
        assert_eq!(n.advance(), Some(-2.0));
        assert_eq!(n.current_gain(), Some(-2.0));
        // Pas de préchargé connu : rien à appliquer après le passage.
        assert_eq!(n.advance(), None);
        n.mode = Normalization::Off;
        assert_eq!(n.loaded(rg((-5.0, 0.5), None)), None);
    }

    #[test]
    fn setting_serialization() {
        assert_eq!(
            serde_json::to_string(&Normalization::Album).unwrap(),
            r#""album""#
        );
        assert!(serde_json::from_str::<Normalization>(r#""loud""#).is_err());
    }

    /// WAV PCM 16 bits stéréo 44,1 kHz : sinus à −40 dBFS (volume bas), fichier de test.
    pub(crate) fn quiet_wav(name: &str, secs: u32, freq: f64) -> std::path::PathBuf {
        let rate = 44_100u32;
        let frames = rate * secs;
        let data_len = frames * 4;
        let mut wav = Vec::with_capacity(44 + data_len as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        for (v, n) in [
            (16u32, 4),
            (1, 2),
            (2, 2),
            (rate, 4),
            (rate * 4, 4),
            (4, 2),
            (16, 2),
        ] {
            wav.extend_from_slice(&v.to_le_bytes()[..n]);
        }
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        for i in 0..frames {
            let s = (0.01
                * 32767.0
                * (std::f64::consts::TAU * freq * f64::from(i) / f64::from(rate)).sin())
                as i16;
            wav.extend_from_slice(&s.to_le_bytes());
            wav.extend_from_slice(&s.to_le_bytes());
        }
        let path = std::env::temp_dir().join(format!("seaglass-{name}-{}.wav", std::process::id()));
        std::fs::write(&path, &wav).unwrap();
        path
    }

    /// Vrai mpv, lecture réelle courte à volume bas (sinus à −40 dBFS, `volume=30`) : le
    /// filtre posé par `loadfile … append -1 af=…` ne vaut que pour le fichier préchargé,
    /// le titre en cours garde le sien jusqu'au passage gapless ; `file-local-options/af`
    /// change celui du titre en cours sans déborder sur les suivants (le préchargé garde
    /// le sien, un fichier chargé ensuite sans option revient à `af=[]`).
    /// `cargo test live_per_file_af -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "lance un vrai mpv sur la sortie audio"]
    async fn live_per_file_af() {
        use crate::mpv::CacheSize;
        use crate::mpv::engine::{af_value, file_options};
        use crate::mpv::events::{MpvEvent, Property};
        use serde_json::{Value, json};
        use std::time::Duration;

        let conf = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("mpv/seaglass.conf");
        let mut child = crate::mpv::process::spawn(&conf, CacheSize::Mib32, None).unwrap();
        let (events_tx, mut events) = tokio::sync::mpsc::channel(256);
        let handle = crate::mpv::ipc::connect(events_tx).await.unwrap();
        let cmd = |c: Vec<Value>| {
            let handle = handle.clone();
            async move { handle.command(c).await.unwrap() }
        };
        let af = || cmd(vec![json!("get_property"), json!("af")]);
        let volume_of = |v: Value| -> Option<String> {
            v.as_array()?
                .iter()
                .find(|f| f["name"] == "volume")
                .and_then(|f| f["params"]["volume"].as_str().map(str::to_owned))
        };
        cmd(vec![json!("set_property"), json!("volume"), json!(30)]).await;

        let a = quiet_wav("af-a", 3, 440.0);
        let b = quiet_wav("af-b", 3, 660.0);
        let c = quiet_wav("af-c", 2, 550.0);
        let path = |p: &std::path::PathBuf| json!(p.to_string_lossy());

        let opts_a = file_options(0.0, Some(-6.0)).unwrap();
        let opts_b = file_options(0.0, Some(-12.0)).unwrap();
        cmd(vec![
            json!("loadfile"),
            path(&a),
            json!("replace"),
            json!(-1),
            json!(opts_a),
        ])
        .await;
        cmd(vec![
            json!("loadfile"),
            path(&b),
            json!("append"),
            json!(-1),
            json!(opts_b),
        ])
        .await;
        tokio::time::sleep(Duration::from_millis(800)).await;
        assert_eq!(
            volume_of(af().await).as_deref(),
            Some("-6.00dB"),
            "titre en cours"
        );
        let params = cmd(vec![json!("get_property"), json!("audio-params")]).await;
        println!("A : af = -6 dB, préchargé B en attente, audio-params = {params}");

        // Changement à chaud du titre en cours : option locale seulement.
        cmd(vec![
            json!("set_property"),
            json!("file-local-options/af"),
            json!(af_value(Some(-3.0))),
        ])
        .await;
        assert_eq!(volume_of(af().await).as_deref(), Some("-3.00dB"));

        // Passage gapless sur B : son propre filtre.
        tokio::time::timeout(Duration::from_secs(6), async {
            while let Some(ev) = events.recv().await {
                if ev == MpvEvent::Property(Property::PlaylistPos(Some(1))) {
                    return;
                }
            }
        })
        .await
        .expect("passage sur le titre préchargé");
        tokio::time::sleep(Duration::from_millis(300)).await;
        let on_b = volume_of(af().await);
        println!("B après passage gapless : {on_b:?}");
        assert_eq!(on_b.as_deref(), Some("-12.00dB"));

        // Fichier sans option : aucun filtre.
        cmd(vec![json!("loadfile"), path(&c), json!("replace")]).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(
            af().await,
            json!([]),
            "sans option, plus de filtre : le changement à chaud n'a pas débordé"
        );

        let _ = child.start_kill();
        for p in [a, b, c] {
            let _ = std::fs::remove_file(p);
        }
    }
}
