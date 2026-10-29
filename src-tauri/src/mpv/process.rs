//! Lancement de mpv avec une configuration isolée de celle de l'utilisateur.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;

use tokio::process::{Child, Command};

use super::{MpvError, Result};

/// `CREATE_NO_WINDOW` : pas de console qui s'ouvre à côté de l'app.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Pipe IPC propre à ce lancement : un mpv orphelin d'un lancement précédent ne peut
/// jamais être pris pour le nôtre.
pub fn pipe_name() -> String {
    format!(r"\\.\pipe\seaglass-mpv-{}", std::process::id())
}

fn executable() -> PathBuf {
    std::env::var_os("SEAGLASS_MPV_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("mpv"))
}

/// Lance mpv en mode idle.
pub fn spawn(conf: &Path) -> Result<Child> {
    let mut cmd = Command::new(executable());
    cmd.arg("--no-config")
        .arg(format!("--include={}", conf.display()))
        .args([
            "--idle=yes",
            "--no-video",
            "--no-terminal",
            "--ao=wasapi",
            "--gapless-audio=weak",
            "--prefetch-playlist=yes",
        ])
        .arg(format!("--input-ipc-server={}", pipe_name()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.spawn().map_err(MpvError::Spawn)
}

/// Détient le processus mpv pour pouvoir l'arrêter à la fermeture de l'app.
///
/// Tauri quitte le processus sans exécuter les destructeurs : `kill_on_drop` ne suffit
/// pas, d'où l'appel explicite à `kill` sur `RunEvent::Exit`.
#[derive(Default)]
pub struct ProcessGuard(Mutex<Option<Child>>);

impl ProcessGuard {
    pub fn set(&self, child: Child) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = Some(child);
        }
    }

    pub fn kill(&self) {
        let Ok(mut slot) = self.0.lock() else {
            return;
        };
        if let Some(mut child) = slot.take()
            && let Err(e) = child.start_kill()
        {
            tracing::warn!("arrêt de mpv impossible : {e}");
        }
    }
}
