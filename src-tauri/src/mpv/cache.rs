//! Size of mpv's cache (#16), chosen in the playback settings.
//!
//! The cache stays **in memory** (principle 1): `cache=yes` and `cache-on-disk=no` are set
//! in `seaglass.conf` and nothing here changes them. Only the demuxer's byte limits change:
//! they are what bounds the read-ahead, since `cache-secs` defaults to a much longer time
//! than they allow.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const MIB: u64 = 1024 * 1024;

/// Read-ahead buffer, in MiB. Only a few values are offered: any other is refused on
/// deserialization (bounds checked on the Rust side).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub enum CacheSize {
    Mib32,
    /// Former value of `seaglass.conf`.
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

    /// `demuxer-max-bytes`: data read ahead of the playback position.
    pub fn max_bytes(self) -> u64 {
        u64::from(self.mib()) * MIB
    }

    /// `demuxer-max-back-bytes`: data kept behind the position, to seek back without
    /// downloading again. A quarter of the forward buffer (64 / 16 MiB originally).
    pub fn back_bytes(self) -> u64 {
        self.max_bytes() / 4
    }

    /// Command-line options, passed when mpv starts.
    pub fn args(self) -> [String; 2] {
        [
            format!("--demuxer-max-bytes={}", self.max_bytes()),
            format!("--demuxer-max-back-bytes={}", self.back_bytes()),
        ]
    }

    /// `set_property` commands that apply the size live, including to the demuxer of the
    /// track already open (effective as soon as playback consumes packets).
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
                "unsupported cache size: {mib} MiB (32, 64, 128 or 256)"
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

    /// Bytes read ahead by the demuxer (`demuxer-cache-state.fw-bytes`).
    async fn fw_bytes(handle: &crate::mpv::MpvHandle) -> u64 {
        let state = handle
            .command(vec![json!("get_property"), json!("demuxer-cache-state")])
            .await
            .unwrap_or(Value::Null);
        state["fw-bytes"].as_u64().unwrap_or(0)
    }

    /// Settled read-ahead (local file: the cache fills in an instant).
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

    /// Real mpv (PATH or `SEAGLASS_MPV_PATH`): the size passed at launch is taken into
    /// account, and a new size set through `set_property` applies to the demuxer of the file
    /// already open. Writes a ~100 MB WAV of silence to the temporary folder (a test file,
    /// not a stream).
    /// `cargo test live_cache_resize -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "starts a real mpv"]
    async fn live_cache_resize() {
        // 16-bit stereo 44.1 kHz PCM WAV, 10 min of silence.
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
        let mut child = crate::mpv::process::spawn(&conf, CacheSize::Mib32, None, false).unwrap();
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

        // No real audio output and no playback: only the cache filling counts.
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
        println!("32 MiB: {before} bytes ahead");
        assert!(before > 16 * MIB && before <= 33 * MIB, "{before}");

        for command in CacheSize::Mib128.set_property_commands() {
            handle.command(command).await.unwrap();
        }
        assert_eq!(get("demuxer-max-bytes").await, json!(128 * MIB));
        // Paused, the full demuxer stays asleep: the new limit applies as soon as playback
        // consumes packets, without a reload.
        handle
            .command(vec![json!("set_property"), json!("pause"), json!(false)])
            .await
            .unwrap();
        let after = settled_fw_bytes(&handle).await;
        println!("128 MiB, live: {after} bytes ahead");
        assert!(after > 64 * MIB, "{after}");

        let _ = child.start_kill();
        let _ = std::fs::remove_file(&path);
    }
}
