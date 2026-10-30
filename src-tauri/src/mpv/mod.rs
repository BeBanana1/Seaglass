//! Pilotage de mpv : processus dédié, JSON IPC sur named pipe, événements typés, et
//! petit serveur HTTP local qui sert les manifestes DASH depuis la mémoire.

pub mod cache;
pub mod device;
pub mod events;
pub mod ipc;
#[cfg(windows)]
mod job;
pub mod process;
pub mod serve;

use crate::i18n::{tr, trf};
pub use cache::CacheSize;
pub use device::AudioDevice;
pub use events::{AudioParams, EndReason, MpvEvent, Property};
pub use ipc::MpvHandle;

#[derive(Debug, thiserror::Error)]
pub enum MpvError {
    #[error("{}", trf!(
        "impossible de lancer mpv ({}) : vérifier le PATH ou SEAGLASS_MPV_PATH",
        "cannot start mpv ({}): check PATH or SEAGLASS_MPV_PATH",
        .0
    ))]
    Spawn(std::io::Error),
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
