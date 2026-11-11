//! Lancement de mpv avec une configuration isolée de celle de l'utilisateur.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use tokio::process::{Child, Command};

use super::device::{AO, AUTO};
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

/// `SEAGLASS_MPV_PATH`, sinon le mpv installé avec l'app. Sous Windows, le repli sur le
/// `mpv` du `PATH` n'existe qu'en debug (#261) : dans un build installé, Windows chercherait
/// aussi dans le dossier courant, et un `mpv.exe` posé là serait lancé à la place de celui de
/// l'installeur. Sous Linux (#17), mpv vient du système et `execvp` ne cherche que dans le
/// `PATH` : le repli vaut aussi en release.
fn executable() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("SEAGLASS_MPV_PATH") {
        return Ok(PathBuf::from(path));
    }
    match bundled() {
        Some(path) => Ok(path),
        None if cfg!(any(debug_assertions, not(windows))) => Ok(PathBuf::from("mpv")),
        None => Err(MpvError::Missing),
    }
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

/// Vrai si `spawn` échoue faute de mpv : build de release sans mpv embarqué ni
/// `SEAGLASS_MPV_PATH` (#261).
pub fn is_missing() -> bool {
    matches!(executable(), Err(MpvError::Missing))
}

/// Lance mpv en mode idle, avec la taille de cache, le périphérique de sortie (`None` =
/// périphérique par défaut du système) et le mode exclusif (#245) choisis dans les
/// réglages. Un périphérique absent est corrigé par le lecteur dès la première liste
/// reçue, avant toute lecture.
pub fn spawn(
    conf: &Path,
    cache: CacheSize,
    audio_device: Option<&str>,
    exclusive: bool,
) -> Result<Child> {
    let exe = executable()?;
    tracing::info!("mpv : {}", exe.display());
    let mut cmd = Command::new(exe);
    cmd.arg("--no-config")
        .arg(format!("--include={}", conf.display()))
        .args([
            "--idle=yes",
            "--no-video",
            "--no-terminal",
            "--gapless-audio=weak",
            "--prefetch-playlist=yes",
        ])
        .arg(format!("--ao={AO}"))
        .args(cache.args())
        .arg(format!("--audio-device={}", audio_device.unwrap_or(AUTO)))
        .arg(super::exclusive::arg(exclusive))
        .arg(format!("--input-ipc-server={}", pipe_name()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    #[cfg(target_os = "linux")]
    die_with_parent(&mut cmd);
    let child = cmd.spawn().map_err(MpvError::Spawn)?;
    #[cfg(windows)]
    if let Some(handle) = child.raw_handle() {
        super::job::adopt(handle);
    }
    Ok(child)
}

/// Équivalent Linux du Job Object (#17) : le noyau envoie `SIGKILL` à mpv quand l'app
/// meurt, plantage compris. Le signal suit la mort du *thread* qui a lancé mpv : `spawn`
/// est appelé depuis une tâche async, donc un thread worker de tokio qui vit autant que
/// l'app (jamais depuis `spawn_blocking`, dont les threads s'arrêtent après 10 s d'inactivité).
#[cfg(target_os = "linux")]
fn die_with_parent(cmd: &mut Command) {
    use std::ffi::{c_int, c_ulong};

    unsafe extern "C" {
        fn prctl(option: c_int, ...) -> c_int;
    }
    const PR_SET_PDEATHSIG: c_int = 1;
    const SIGKILL: c_ulong = 9;
    const ESRCH: i32 = 3;

    let parent = std::process::id();
    // SAFETY : la fermeture s'exécute entre `fork` et `exec` ; elle n'appelle que `prctl` et
    // `getppid` (sûrs après `fork`) et n'alloue pas. Si l'app est morte avant `prctl`, mpv
    // a déjà été rattaché à un autre parent : on abandonne le lancement.
    unsafe {
        cmd.pre_exec(move || {
            if prctl(PR_SET_PDEATHSIG, SIGKILL) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if std::os::unix::process::parent_id() != parent {
                return Err(std::io::Error::from_raw_os_error(ESRCH));
            }
            Ok(())
        });
    }
}

/// Détient le processus mpv pour pouvoir l'arrêter à la fermeture de l'app.
///
/// Tauri quitte le processus sans exécuter les destructeurs : `kill_on_drop` ne suffit
/// pas, d'où l'appel explicite à `kill` sur `RunEvent::Exit` (et avant l'installation
/// d'une mise à jour). Sur un plantage, c'est le Job Object (`job.rs`) qui arrête mpv, ou
/// `PR_SET_PDEATHSIG` sous Linux.
#[derive(Default)]
pub struct ProcessGuard {
    child: Mutex<Option<Child>>,
    /// Arrêt voulu (fermeture de l'app, mise à jour) : mpv ne doit pas être relancé (#255).
    closing: AtomicBool,
}

impl ProcessGuard {
    pub fn set(&self, child: Child) {
        if let Ok(mut slot) = self.child.lock() {
            // Ancien processus d'avant une relance, déjà arrêté en principe.
            if let Some(mut old) = slot.replace(child) {
                let _ = old.start_kill();
            }
        }
    }

    /// Arrêt définitif : fermeture de l'app ou installation d'une mise à jour.
    pub fn kill(&self) {
        self.closing.store(true, Ordering::SeqCst);
        self.terminate();
    }

    /// Arrête mpv sans fermer l'app : il ne répond plus, l'acteur va le relancer (#255).
    pub fn terminate(&self) {
        let Ok(mut slot) = self.child.lock() else {
            return;
        };
        if let Some(mut child) = slot.take()
            && let Err(e) = child.start_kill()
        {
            tracing::warn!("arrêt de mpv impossible : {e}");
        }
    }

    pub fn closing(&self) -> bool {
        self.closing.load(Ordering::SeqCst)
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
