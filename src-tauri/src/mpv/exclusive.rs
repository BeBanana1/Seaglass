//! WASAPI exclusive mode (#245): mpv opens the device for itself alone, bypassing Windows'
//! mixer. Pure logic here; the actor decides when to turn it on.

/// Value of `--audio-exclusive` / `audio-exclusive`.
pub fn arg(exclusive: bool) -> String {
    format!("--audio-exclusive={}", if exclusive { "yes" } else { "no" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_flag() {
        assert_eq!(arg(true), "--audio-exclusive=yes");
        assert_eq!(arg(false), "--audio-exclusive=no");
    }

    /// Real mpv on the real WASAPI output, sine at −40 dBFS and volume 30:
    /// - in shared mode, `audio-out-params` is Windows' mix format;
    /// - `audio-exclusive` changes live, without interrupting the track;
    /// - in exclusive mode, `audio-out-params` gives the format actually opened;
    /// - another player in exclusive mode on the same device fails;
    /// - device held by another player: switching to exclusive, then loading a track, both
    ///   end in `end-file` `error` [`INIT_FAILED`].
    ///
    /// `cargo test live_exclusive -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "starts a real mpv on the audio output, in exclusive mode"]
    async fn live_exclusive() {
        use std::time::Duration;

        use serde_json::{Value, json};

        use crate::mpv::CacheSize;
        use crate::mpv::device::INIT_FAILED;
        use crate::mpv::events::{EndReason, MpvEvent, Property};
        use crate::player::normalize::tests::quiet_wav;

        let conf = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("mpv/seaglass.conf");
        let mut child = crate::mpv::process::spawn(&conf, CacheSize::Mib32, None, false).unwrap();
        let (events_tx, mut events) = tokio::sync::mpsc::channel(256);
        let handle = crate::mpv::ipc::connect(events_tx).await.unwrap();
        let cmd = |c: Vec<Value>| {
            let handle = handle.clone();
            async move { handle.command(c).await }
        };
        async fn out_params(
            events: &mut tokio::sync::mpsc::Receiver<MpvEvent>,
        ) -> crate::engine::AudioParams {
            tokio::time::timeout(Duration::from_secs(10), async {
                while let Some(ev) = events.recv().await {
                    if let MpvEvent::Property(Property::AudioOutParams(Some(p))) = ev {
                        return p;
                    }
                }
                panic!("mpv closed");
            })
            .await
            .expect("audio-out-params attendu")
        }
        let pos = |v: Value| v.as_f64().unwrap_or(0.0);

        let wav = quiet_wav("exclusive", 8, 440.0);
        cmd(vec![json!("set_property"), json!("volume"), json!(30)])
            .await
            .unwrap();
        cmd(vec![json!("loadfile"), json!(wav.to_string_lossy())])
            .await
            .unwrap();
        let shared = out_params(&mut events).await;
        println!("shared: {shared:?}");

        tokio::time::sleep(Duration::from_millis(1000)).await;
        let before = pos(cmd(vec![json!("get_property"), json!("time-pos")])
            .await
            .unwrap());
        cmd(vec![
            json!("set_property"),
            json!("audio-exclusive"),
            json!(true),
        ])
        .await
        .unwrap();
        let exclusive = out_params(&mut events).await;
        println!("exclusif : {exclusive:?}");
        tokio::time::sleep(Duration::from_millis(1000)).await;
        let after = pos(cmd(vec![json!("get_property"), json!("time-pos")])
            .await
            .unwrap());
        println!("position before the switch {before:.2} s, after {after:.2} s");
        assert!(after > before + 0.5, "{before} → {after}");

        // Another player claims the same device in exclusive mode.
        let other = std::process::Command::new(
            std::env::var_os("SEAGLASS_MPV_PATH").unwrap_or_else(|| "mpv".into()),
        )
        .args([
            "--no-config",
            "--no-video",
            "--ao=wasapi",
            "--audio-exclusive=yes",
            "--volume=0",
            "--msg-level=all=error",
        ])
        .arg(wav.to_string_lossy().as_ref())
        .output()
        .unwrap();
        println!("autre lecteur : {}", other.status);
        assert!(
            !other.status.success(),
            "the device was taken in exclusive mode"
        );

        cmd(vec![
            json!("set_property"),
            json!("audio-exclusive"),
            json!(false),
        ])
        .await
        .unwrap();
        println!("back to shared: {:?}", out_params(&mut events).await);

        // The other player holds the device in exclusive mode: switching to exclusive in turn,
        // our mpv sees the current track fail, then a new track too.
        let mut holder = std::process::Command::new(
            std::env::var_os("SEAGLASS_MPV_PATH").unwrap_or_else(|| "mpv".into()),
        )
        .args([
            "--no-config",
            "--no-video",
            "--ao=wasapi",
            "--audio-exclusive=yes",
            "--volume=0",
            "--loop-file=inf",
        ])
        .arg(wav.to_string_lossy().as_ref())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
        tokio::time::sleep(Duration::from_millis(1500)).await;
        while events.try_recv().is_ok() {}
        async fn end_file(events: &mut tokio::sync::mpsc::Receiver<MpvEvent>) -> Vec<MpvEvent> {
            let mut seen = Vec::new();
            let _ = tokio::time::timeout(Duration::from_secs(4), async {
                while let Some(ev) = events.recv().await {
                    let done = matches!(ev, MpvEvent::EndFile { .. });
                    seen.push(ev);
                    if done {
                        break;
                    }
                }
            })
            .await;
            seen
        }
        cmd(vec![
            json!("set_property"),
            json!("audio-exclusive"),
            json!(true),
        ])
        .await
        .unwrap();
        let failed = MpvEvent::EndFile {
            reason: EndReason::Error,
            error: Some(INIT_FAILED.into()),
        };
        let seen = end_file(&mut events).await;
        println!("switch, device taken: {seen:?}");
        assert_eq!(seen.last(), Some(&failed));
        cmd(vec![json!("loadfile"), json!(wav.to_string_lossy())])
            .await
            .unwrap();
        let seen = end_file(&mut events).await;
        println!("new track, device taken: {seen:?}");
        assert_eq!(seen.last(), Some(&failed));
        let _ = holder.kill();
        let _ = holder.wait();

        let _ = child.start_kill();
        let _ = std::fs::remove_file(&wav);
    }
}
