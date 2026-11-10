//! Pilotage de mpv : processus dédié, JSON IPC sur named pipe, événements typés, et
//! petit serveur HTTP local qui sert les manifestes DASH depuis la mémoire.

// Sur Android (#24), seuls le serveur de manifestes et les types de réglages servent :
// mpv n'y est jamais lancé.
#![cfg_attr(mobile, allow(dead_code))]

pub mod cache;
pub mod cpu;
pub mod device;
pub mod engine;
pub mod events;
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

/// Pourquoi mpv ne tourne pas : affiché en bandeau par l'UI tant que l'app tourne (#92).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MpvIssue {
    /// mpv n'a pas démarré, ou s'est arrêté.
    Stopped,
    /// mpv de l'installeur (x86-64-v3) sur un processeur sans AVX2 : il ne peut pas tourner.
    CpuUnsupported,
    /// Build installé sans son `mpv.exe` (installation abîmée, antivirus) : pas de repli sur
    /// le `PATH` hors debug (#261).
    Missing,
}

impl MpvIssue {
    /// Cause probable d'un échec de mpv sur cette machine.
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

    /// Message des toasts (lecture demandée alors que mpv ne tourne pas).
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
    /// Build installé sans le `mpv.exe` de l'installeur ni `SEAGLASS_MPV_PATH` (#261).
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
        // mpv choisi par l'utilisateur (SEAGLASS_MPV_PATH, PATH) : peut-être une build v1.
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
