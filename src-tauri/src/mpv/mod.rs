//! Pilotage de mpv : processus dédié, JSON IPC sur named pipe, événements typés, et
//! petit serveur HTTP local qui sert les manifestes DASH depuis la mémoire.

pub mod events;
pub mod ipc;
#[cfg(windows)]
mod job;
pub mod process;
pub mod serve;

pub use events::{AudioParams, EndReason, MpvEvent, Property};
pub use ipc::MpvHandle;

#[derive(Debug, thiserror::Error)]
pub enum MpvError {
    #[error("impossible de lancer mpv ({0}) : vérifier le PATH ou SEAGLASS_MPV_PATH")]
    Spawn(std::io::Error),
    #[error("connexion IPC à mpv impossible : {0}")]
    Connect(std::io::Error),
    #[error("mpv s'est arrêté")]
    Closed,
    #[error("mpv a refusé la commande : {0}")]
    Command(String),
    #[error("serveur de manifestes : {0}")]
    Serve(std::io::Error),
}

pub type Result<T> = std::result::Result<T, MpvError>;
