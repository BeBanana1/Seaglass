//! Lancement de mpv avec une configuration isolée de celle de l'utilisateur.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::{Child, Command};

use super::{MpvError, Result};

pub const PIPE_NAME: &str = r"\\.\pipe\seaglass-mpv";

/// `CREATE_NO_WINDOW` : pas de console qui s'ouvre à côté de l'app.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn executable() -> PathBuf {
    std::env::var_os("SEAGLASS_MPV_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("mpv"))
}

/// Lance mpv en mode idle. Le processus est tué si le `Child` est abandonné.
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
        .arg(format!("--input-ipc-server={PIPE_NAME}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.spawn().map_err(MpvError::Spawn)
}
