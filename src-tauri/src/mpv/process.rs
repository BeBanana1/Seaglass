//! Lancement de mpv avec une configuration isolée de celle de l'utilisateur.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;

use tokio::process::{Child, Command};

use super::{MpvError, Result};

/// `CREATE_NO_WINDOW` : pas de console qui s'ouvre à côté de l'app.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Nom de l'exécutable mpv livré par l'installeur, à côté de celui de l'app.
#[cfg(windows)]
const BUNDLED_MPV: &str = "mpv.exe";
#[cfg(not(windows))]
const BUNDLED_MPV: &str = "mpv";

/// Pipe IPC propre à ce lancement : un mpv orphelin d'un lancement précédent ne peut
/// jamais être pris pour le nôtre.
pub fn pipe_name() -> String {
    format!(r"\\.\pipe\seaglass-mpv-{}", std::process::id())
}

/// `SEAGLASS_MPV_PATH`, sinon le mpv installé avec l'app, sinon celui du `PATH` (dev).
fn executable() -> PathBuf {
    if let Some(path) = std::env::var_os("SEAGLASS_MPV_PATH") {
        return PathBuf::from(path);
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(BUNDLED_MPV)))
        .filter(|bundled| bundled.is_file())
        .unwrap_or_else(|| PathBuf::from("mpv"))
}

/// Lance mpv en mode idle.
pub fn spawn(conf: &Path) -> Result<Child> {
    let exe = executable();
    tracing::info!("mpv : {}", exe.display());
    let mut cmd = Command::new(exe);
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
    let child = cmd.spawn().map_err(MpvError::Spawn)?;
    #[cfg(windows)]
    if let Some(handle) = child.raw_handle() {
        super::job::adopt(handle);
    }
    Ok(child)
}

/// Détient le processus mpv pour pouvoir l'arrêter à la fermeture de l'app.
///
/// Tauri quitte le processus sans exécuter les destructeurs : `kill_on_drop` ne suffit
/// pas, d'où l'appel explicite à `kill` sur `RunEvent::Exit` (et avant l'installation
/// d'une mise à jour). Sur un plantage, c'est le Job Object (`job.rs`) qui arrête mpv.
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
