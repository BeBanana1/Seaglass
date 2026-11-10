//! Mode exclusif WASAPI (#245) : mpv ouvre le périphérique pour lui seul, sans passer par
//! le mixeur de Windows. Logique pure ici ; l'acteur décide quand l'activer.

/// Valeur de `--audio-exclusive` / `audio-exclusive`.
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

    /// Vrai mpv sur la vraie sortie WASAPI, sinus à −40 dBFS et volume 30 :
    /// - en partagé, `audio-out-params` est le format de mixage de Windows ;
    /// - `audio-exclusive` se change à chaud, sans interrompre le titre ;
    /// - en exclusif, `audio-out-params` donne le format réellement ouvert ;
    /// - un autre lecteur en exclusif sur le même périphérique échoue ;
    /// - périphérique tenu par un autre lecteur : passer en exclusif, puis charger un titre,
    ///   finissent tous deux en `end-file` `error` [`INIT_FAILED`].
    ///
    /// `cargo test live_exclusive -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "lance un vrai mpv sur la sortie audio, en exclusif"]
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
                panic!("mpv fermé");
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
        println!("partagé : {shared:?}");

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
        println!("position avant bascule {before:.2} s, après {after:.2} s");
        assert!(after > before + 0.5, "{before} → {after}");

        // Un autre lecteur réclame le même périphérique en exclusif.
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
            "le périphérique était pris en exclusif"
        );

        cmd(vec![
            json!("set_property"),
            json!("audio-exclusive"),
            json!(false),
        ])
        .await
        .unwrap();
        println!("retour en partagé : {:?}", out_params(&mut events).await);

        // L'autre lecteur tient le périphérique en exclusif : en exclusif à son tour, notre
        // mpv voit le titre en cours échouer, puis un nouveau titre aussi.
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
        println!("bascule, périphérique pris : {seen:?}");
        assert_eq!(seen.last(), Some(&failed));
        cmd(vec![json!("loadfile"), json!(wav.to_string_lossy())])
            .await
            .unwrap();
        let seen = end_file(&mut events).await;
        println!("nouveau titre, périphérique pris : {seen:?}");
        assert_eq!(seen.last(), Some(&failed));
        let _ = holder.kill();
        let _ = holder.wait();

        let _ = child.start_kill();
        let _ = std::fs::remove_file(&wav);
    }
}
