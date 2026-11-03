//! Lancement de mpv avec une configuration isolée de celle de l'utilisateur.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};

use tokio::process::{Child, Command};

use super::device::AUTO;
use super::{CacheSize, MpvError, Result};

/// `CREATE_NO_WINDOW` : pas de console qui s'ouvre à côté de l'app.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Nom de l'exécutable mpv livré par l'installeur, à côté de celui de l'app.
#[cfg(windows)]
const BUNDLED_MPV: &str = "mpv.exe";
#[cfg(not(windows))]
const BUNDLED_MPV: &str = "mpv";

/// `<pid>-<128 bits aléatoires>`, tiré une fois par lancement : le pid écarte un mpv
/// orphelin d'un lancement précédent, la partie aléatoire empêche un autre processus de
/// créer le pipe avant mpv pour recevoir la connexion de l'app.
fn pipe_id() -> &'static str {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| format!("{}-{}", std::process::id(), super::serve::random_token()))
}

/// Pipe IPC propre à ce lancement, partagé par `spawn` et `ipc::connect`.
#[cfg(windows)]
pub fn pipe_name() -> String {
    format!(r"\\.\pipe\seaglass-mpv-{}", pipe_id())
}

/// Hors Windows, mpv attend le chemin d'un socket Unix.
#[cfg(not(windows))]
pub fn pipe_name() -> String {
    std::env::temp_dir()
        .join(format!("seaglass-mpv-{}.sock", pipe_id()))
        .to_string_lossy()
        .into_owned()
}

/// `SEAGLASS_MPV_PATH`, sinon le mpv installé avec l'app, sinon celui du `PATH` (dev).
fn executable() -> PathBuf {
    if let Some(path) = std::env::var_os("SEAGLASS_MPV_PATH") {
        return PathBuf::from(path);
    }
    bundled().unwrap_or_else(|| PathBuf::from("mpv"))
}

/// Le mpv installé avec l'app, s'il existe.
fn bundled() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(BUNDLED_MPV)))
        .filter(|bundled| bundled.is_file())
}

/// Vrai si `spawn` lance le mpv de l'installeur (build x86-64-v3, voir `cpu.rs`).
pub fn is_bundled() -> bool {
    std::env::var_os("SEAGLASS_MPV_PATH").is_none() && bundled().is_some()
}

/// Lance mpv en mode idle, avec la taille de cache et le périphérique de sortie choisis
/// dans les réglages (`None` = périphérique par défaut du système). Un périphérique absent
/// est corrigé par le lecteur dès la première liste reçue, avant toute lecture.
pub fn spawn(conf: &Path, cache: CacheSize, audio_device: Option<&str>) -> Result<Child> {
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
        .args(cache.args())
        .arg(format!("--audio-device={}", audio_device.unwrap_or(AUTO)))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_name_is_stable_and_unguessable() {
        let name = pipe_name();
        assert_eq!(name, pipe_name());
        let pid = std::process::id().to_string();
        let suffix = name.split(&format!("seaglass-mpv-{pid}-")).nth(1).unwrap();
        assert!(suffix.len() >= 32, "{name}");
    }
}
