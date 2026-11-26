//! Starting mpv with a config kept apart from the user's own.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use tokio::process::{Child, Command};

use super::device::{AO, AUTO};
use super::{CacheSize, MpvError, Result};

/// `CREATE_NO_WINDOW`: no console opening next to the app.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Name of the mpv executable shipped by the installer, next to the app's.
#[cfg(windows)]
const BUNDLED_MPV: &str = "mpv.exe";
#[cfg(not(windows))]
const BUNDLED_MPV: &str = "mpv";

/// `<pid>-<128 random bits>`, drawn once per launch: the pid rules out an orphan mpv from
/// a previous launch, the random part prevents another process from creating the pipe
/// before mpv to receive the app's connection.
fn pipe_id() -> &'static str {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| format!("{}-{}", std::process::id(), super::serve::random_token()))
}

/// IPC pipe of this launch, shared by `spawn` and `ipc::connect`.
#[cfg(windows)]
pub fn pipe_name() -> String {
    format!(r"\\.\pipe\seaglass-mpv-{}", pipe_id())
}

/// Outside Windows, mpv expects the path of a Unix socket.
#[cfg(not(windows))]
pub fn pipe_name() -> String {
    std::env::temp_dir()
        .join(format!("seaglass-mpv-{}.sock", pipe_id()))
        .to_string_lossy()
        .into_owned()
}

/// `SEAGLASS_MPV_PATH`, otherwise the mpv installed with the app. On Windows, the fallback
/// to the `mpv` in `PATH` only exists in debug (#261): in an installed build, Windows would
/// also search the current folder, and an `mpv.exe` placed there would start instead of
/// the installer's. On Linux (#17), mpv comes from the system and `execvp` only searches
/// `PATH`: the fallback also applies in release.
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

/// The mpv installed with the app, if there is one.
fn bundled() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(BUNDLED_MPV)))
        .filter(|bundled| bundled.is_file())
}

/// True if `spawn` starts the installer's mpv (x86-64-v3 build, see `cpu.rs`).
pub fn is_bundled() -> bool {
    std::env::var_os("SEAGLASS_MPV_PATH").is_none() && bundled().is_some()
}

/// True if `spawn` fails for lack of mpv: a release build with no bundled mpv nor
/// `SEAGLASS_MPV_PATH` (#261).
pub fn is_missing() -> bool {
    matches!(executable(), Err(MpvError::Missing))
}

/// Starts mpv in idle mode, with the cache size, the output device (`None` = the system's
/// default device) and the exclusive mode (#245) chosen in the settings. The player fixes a
/// missing device as soon as the first list arrives, before any playback.
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
    {
        die_with_parent(&mut cmd);
        clean_appimage_env(&mut cmd);
    }
    let child = cmd.spawn().map_err(MpvError::Spawn)?;
    #[cfg(windows)]
    if let Some(handle) = child.raw_handle() {
        super::job::adopt(handle);
    }
    Ok(child)
}

/// Started from the AppImage (`APPDIR` set), the app would pass the AppImage's library
/// and plugin paths on to mpv, a system binary (#320).
#[cfg(target_os = "linux")]
fn clean_appimage_env(cmd: &mut Command) {
    let Some(appdir) = std::env::var_os("APPDIR") else {
        return;
    };
    // Non-UTF-8 variables left as they are: the AppRun sets none.
    let vars = std::env::vars_os()
        .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)));
    let fixes = super::appimage_env::fixes(vars, &appdir.to_string_lossy());
    if !fixes.is_empty() {
        tracing::info!("mpv: {} AppImage variable(s) fixed", fixes.len());
    }
    for fix in fixes {
        match fix {
            super::appimage_env::Fix::Remove(key) => {
                cmd.env_remove(key);
            }
            super::appimage_env::Fix::Set(key, value) => {
                cmd.env(key, value);
            }
        }
    }
}

/// Linux equivalent of the Job Object (#17): the kernel sends `SIGKILL` to mpv when the
/// app dies, crashes included. The signal follows the death of the *thread* that started
/// mpv: `spawn` is called from an async task, so a tokio worker thread that lives as long as
/// the app (never from `spawn_blocking`, whose threads stop after 10 s of inactivity).
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
    // SAFETY: the closure runs between `fork` and `exec`; it only calls `prctl` and
    // `getppid` (safe after `fork`) and does not allocate. If the app died before `prctl`,
    // mpv was already reparented: the launch is abandoned.
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

/// Holds the mpv process so it can be stopped when the app closes.
///
/// Tauri exits the process without running destructors: `kill_on_drop` is not enough,
/// hence the explicit call to `kill` on `RunEvent::Exit` (and before an update is
/// installed). On a crash, the Job Object (`job.rs`) stops mpv, or
/// `PR_SET_PDEATHSIG` on Linux.
#[derive(Default)]
pub struct ProcessGuard {
    child: Mutex<Option<Child>>,
    /// Stop on purpose (app closing, update): mpv must not be restarted (#255).
    closing: AtomicBool,
}

impl ProcessGuard {
    pub fn set(&self, child: Child) {
        if let Ok(mut slot) = self.child.lock() {
            // Former process from before a restart, normally already stopped: its exit code
            // goes to the log (#192), where an mpv crash stands out from a stop on purpose.
            if let Some(mut old) = slot.replace(child) {
                match old.try_wait() {
                    Ok(Some(status)) => tracing::warn!("mpv had stopped ({status})"),
                    Ok(None) => tracing::warn!("former mpv still running, stopped"),
                    Err(e) => tracing::warn!("cannot read the former mpv's state: {e}"),
                }
                let _ = old.start_kill();
            }
        }
    }

    /// Final stop: the app closing or an update being installed.
    pub fn kill(&self) {
        self.closing.store(true, Ordering::SeqCst);
        self.terminate();
    }

    /// Stops mpv without closing the app: it no longer answers, the actor will restart it (#255).
    pub fn terminate(&self) {
        let Ok(mut slot) = self.child.lock() else {
            return;
        };
        if let Some(mut child) = slot.take()
            && let Err(e) = child.start_kill()
        {
            tracing::warn!("cannot stop mpv: {e}");
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
