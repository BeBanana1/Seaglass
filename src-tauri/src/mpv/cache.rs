//! Taille du cache de mpv (#16), choisie dans les réglages de lecture.
//!
//! Le cache reste **en mémoire** (principe 1) : `cache=yes` et `cache-on-disk=no` sont
//! fixés dans `seaglass.conf` et rien ici ne les modifie. Seules les limites en octets
//! du démuxeur changent : c'est elles qui bornent la lecture en avance, `cache-secs`
//! valant par défaut une durée bien plus longue que ce qu'elles permettent.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const MIB: u64 = 1024 * 1024;

/// Tampon de lecture en avance, en Mio. Seules quelques valeurs sont proposées : toute
/// autre est refusée à la désérialisation (bornes validées côté Rust).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub enum CacheSize {
    Mib32,
    /// Valeur historique de `seaglass.conf`.
    #[default]
    Mib64,
    Mib128,
    Mib256,
}

impl CacheSize {
    pub fn mib(self) -> u32 {
        match self {
            Self::Mib32 => 32,
            Self::Mib64 => 64,
            Self::Mib128 => 128,
            Self::Mib256 => 256,
        }
    }

    /// `demuxer-max-bytes` : données lues en avance de la position de lecture.
    pub fn max_bytes(self) -> u64 {
        u64::from(self.mib()) * MIB
    }

    /// `demuxer-max-back-bytes` : données gardées derrière la position, pour revenir en
    /// arrière sans retélécharger. Un quart du tampon avant (64 / 16 Mio à l'origine).
    pub fn back_bytes(self) -> u64 {
        self.max_bytes() / 4
    }

    /// Options de ligne de commande, passées au lancement de mpv.
    pub fn args(self) -> [String; 2] {
        [
            format!("--demuxer-max-bytes={}", self.max_bytes()),
            format!("--demuxer-max-back-bytes={}", self.back_bytes()),
        ]
    }

    /// Commandes `set_property` qui appliquent la taille à chaud, y compris au démuxeur
    /// du titre déjà ouvert (effective dès que la lecture consomme des paquets).
    pub fn set_property_commands(self) -> [Vec<Value>; 2] {
        [
            vec![
                json!("set_property"),
                json!("demuxer-max-bytes"),
                json!(self.max_bytes()),
            ],
            vec![
                json!("set_property"),
                json!("demuxer-max-back-bytes"),
                json!(self.back_bytes()),
            ],
        ]
    }
}

impl TryFrom<u32> for CacheSize {
    type Error = String;

    fn try_from(mib: u32) -> Result<Self, Self::Error> {
        match mib {
            32 => Ok(Self::Mib32),
            64 => Ok(Self::Mib64),
            128 => Ok(Self::Mib128),
            256 => Ok(Self::Mib256),
            _ => Err(format!(
                "taille de cache non prise en charge : {mib} Mio (32, 64, 128 ou 256)"
            )),
        }
    }
}

impl From<CacheSize> for u32 {
    fn from(size: CacheSize) -> Self {
        size.mib()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_presets_deserialize() {
        for mib in [32, 64, 128, 256] {
            let size: CacheSize = serde_json::from_str(&mib.to_string()).unwrap();
            assert_eq!(size.mib(), mib);
            assert_eq!(serde_json::to_string(&size).unwrap(), mib.to_string());
        }
        for bad in ["0", "16", "100", "512", "-64", "\"64\"", "64.5"] {
            assert!(serde_json::from_str::<CacheSize>(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn default_matches_previous_config() {
        assert_eq!(CacheSize::default().max_bytes(), 64 * MIB);
        assert_eq!(CacheSize::default().back_bytes(), 16 * MIB);
        assert_eq!(
            CacheSize::Mib256.args(),
            [
                "--demuxer-max-bytes=268435456".to_owned(),
                "--demuxer-max-back-bytes=67108864".to_owned(),
            ]
        );
    }

    /// Octets lus en avance par le démuxeur (`demuxer-cache-state.fw-bytes`).
    async fn fw_bytes(handle: &crate::mpv::MpvHandle) -> u64 {
        let state = handle
            .command(vec![json!("get_property"), json!("demuxer-cache-state")])
            .await
            .unwrap_or(Value::Null);
        state["fw-bytes"].as_u64().unwrap_or(0)
    }

    /// Lecture en avance stabilisée (fichier local : le cache se remplit en un instant).
    async fn settled_fw_bytes(handle: &crate::mpv::MpvHandle) -> u64 {
        let mut last = 0;
        for _ in 0..40 {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            let now = fw_bytes(handle).await;
            if now > 0 && now == last {
                return now;
            }
            last = now;
        }
        last
    }

    /// Vrai mpv (PATH ou `SEAGLASS_MPV_PATH`) : la taille passée au lancement est prise
    /// en compte, et une nouvelle taille posée par `set_property` s'applique au démuxeur
    /// du fichier déjà ouvert. Écrit un WAV de silence de ~100 Mo dans le dossier
    /// temporaire (fichier de test, pas un flux).
    /// `cargo test live_cache_resize -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "lance un vrai mpv"]
    async fn live_cache_resize() {
        // WAV PCM 16 bits stéréo 44,1 kHz, 10 min de silence.
        let data_len: u32 = 44_100 * 4 * 600;
        let mut wav = Vec::with_capacity(44 + data_len as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&44_100u32.to_le_bytes());
        wav.extend_from_slice(&(44_100u32 * 4).to_le_bytes());
        wav.extend_from_slice(&4u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        wav.resize(44 + data_len as usize, 0);
        let path = std::env::temp_dir().join(format!("seaglass-cache-{}.wav", std::process::id()));
        std::fs::write(&path, &wav).unwrap();

        let conf = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("mpv/seaglass.conf");
        let mut child = crate::mpv::process::spawn(&conf, CacheSize::Mib32).unwrap();
        let (events_tx, mut events_rx) = tokio::sync::mpsc::channel(256);
        tokio::spawn(async move { while events_rx.recv().await.is_some() {} });
        let handle = crate::mpv::ipc::connect(events_tx).await.unwrap();

        let get = |name: &'static str| {
            let handle = handle.clone();
            async move {
                handle
                    .command(vec![json!("get_property"), json!(name)])
                    .await
                    .unwrap()
            }
        };
        assert_eq!(get("demuxer-max-bytes").await, json!(32 * MIB));
        assert_eq!(get("demuxer-max-back-bytes").await, json!(8 * MIB));
        assert_eq!(get("cache-on-disk").await, json!(false));

        // Ni sortie audio réelle ni lecture : seul le remplissage du cache compte.
        for (name, value) in [("ao", json!("null")), ("pause", json!(true))] {
            handle
                .command(vec![json!("set_property"), json!(name), value])
                .await
                .unwrap();
        }
        handle
            .command(vec![json!("loadfile"), json!(path.to_string_lossy())])
            .await
            .unwrap();
        let before = settled_fw_bytes(&handle).await;
        println!("32 Mio : {before} octets en avance");
        assert!(before > 16 * MIB && before <= 33 * MIB, "{before}");

        for command in CacheSize::Mib128.set_property_commands() {
            handle.command(command).await.unwrap();
        }
        assert_eq!(get("demuxer-max-bytes").await, json!(128 * MIB));
        // En pause, le démuxeur plein reste endormi : la nouvelle limite est prise en
        // compte dès que la lecture consomme des paquets, sans rechargement.
        handle
            .command(vec![json!("set_property"), json!("pause"), json!(false)])
            .await
            .unwrap();
        let after = settled_fw_bytes(&handle).await;
        println!("128 Mio, à chaud : {after} octets en avance");
        assert!(after > 64 * MIB, "{after}");

        let _ = child.start_kill();
        let _ = std::fs::remove_file(&path);
    }
}
