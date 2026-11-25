//! mpv control: dedicated process, JSON IPC over a named pipe, typed events, and a small
//! local HTTP server that serves the DASH manifests from memory.

// On Android (#24), only the manifest server and the settings types are used: mpv is
// never launched there.
#![cfg_attr(mobile, allow(dead_code))]

#[cfg(any(target_os = "linux", test))]
mod appimage_env;
pub mod cache;
pub mod cpu;
pub mod device;
pub mod engine;
pub mod events;
pub mod exclusive;
pub mod filters;
pub mod ipc;
#[cfg(windows)]
mod job;
pub mod process;
pub mod serve;

use serde::Serialize;

use crate::i18n::{tr, trf};
pub use cache::CacheSize;
pub use device::AudioDevice;

pub use ipc::MpvHandle;

/// Why mpv is not running: shown as a banner by the UI while the app runs (#92).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export, export_to = "../../src/lib/bindings/")
)]
pub enum MpvIssue {
    /// mpv did not start, or stopped.
    Stopped,
    /// The installer's mpv (x86-64-v3) on a processor without AVX2: it cannot run.
    CpuUnsupported,
    /// Installed build without its `mpv.exe` (damaged install, antivirus): no fallback to the
    /// `PATH` outside debug (#261).
    Missing,
}

impl MpvIssue {
    /// Probable cause of an mpv failure on this machine.
    pub fn diagnose() -> Self {
        if process::is_missing() {
            return Self::Missing;
        }
        Self::classify(process::is_bundled(), cpu::lacks_avx2())
    }

    fn classify(bundled: bool, lacks_avx2: bool) -> Self {
        if bundled && lacks_avx2 {
            Self::CpuUnsupported
        } else {
            Self::Stopped
        }
    }

    /// Toast message (playback asked while mpv is not running).
    pub fn message(self) -> &'static str {
        match self {
            Self::Stopped => tr(
                "mpv s'est arrêté, redémarrer l'app",
                "mpv stopped, restart the app",
            ),
            Self::CpuUnsupported => tr(
                "lecture impossible : ce processeur n'a pas l'AVX2, exigé par le mpv livré avec Seaglass",
                "playback unavailable: this processor lacks AVX2, which the mpv bundled with Seaglass requires",
            ),
            Self::Missing => tr(
                "lecture impossible : mpv introuvable à côté de Seaglass, réinstaller l'app",
                "playback unavailable: mpv not found next to Seaglass, reinstall the app",
            ),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MpvError {
    #[error("{}", trf!(
        "impossible de lancer mpv ({}) : vérifier SEAGLASS_MPV_PATH ou réinstaller l'app",
        "cannot start mpv ({}): check SEAGLASS_MPV_PATH or reinstall the app",
        .0
    ))]
    Spawn(std::io::Error),
    /// Installed build without the installer's `mpv.exe` nor `SEAGLASS_MPV_PATH` (#261).
    #[error(
        "{}",
        tr(
            "mpv introuvable à côté de Seaglass : réinstaller l'app",
            "mpv not found next to Seaglass: reinstall the app"
        )
    )]
    Missing,
    #[error("{}", trf!("connexion IPC à mpv impossible : {}", "cannot connect to mpv over IPC: {}", .0))]
    Connect(std::io::Error),
    #[error("{}", tr("mpv s'est arrêté", "mpv stopped"))]
    Closed,
    #[error("{}", trf!("mpv a refusé la commande : {}", "mpv rejected the command: {}", .0))]
    Command(String),
    #[error("{}", trf!("serveur de manifestes : {}", "manifest server: {}", .0))]
    Serve(std::io::Error),
}

pub type Result<T> = std::result::Result<T, MpvError>;

#[cfg(test)]
mod tests {
    use super::MpvIssue;

    #[test]
    fn avx2_is_blamed_only_for_the_bundled_mpv() {
        assert_eq!(MpvIssue::classify(true, true), MpvIssue::CpuUnsupported);
        // mpv chosen by the user (SEAGLASS_MPV_PATH, PATH): maybe a v1 build.
        assert_eq!(MpvIssue::classify(false, true), MpvIssue::Stopped);
        assert_eq!(MpvIssue::classify(true, false), MpvIssue::Stopped);
    }

    #[test]
    fn issue_is_serialized_in_camel_case() {
        assert_eq!(
            serde_json::to_string(&MpvIssue::CpuUnsupported).unwrap(),
            "\"cpuUnsupported\""
        );
    }
}
