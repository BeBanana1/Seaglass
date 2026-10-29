//! Commandes Tauri : valident l'entrée puis délèguent à `tidal` ou envoient un `Cmd`.

pub mod auth;
pub mod catalog;
pub mod player;

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::player::Cmd;
use crate::tidal::TidalClient;

pub struct AppState {
    pub tidal: Arc<TidalClient>,
    pub player: mpsc::Sender<Cmd>,
}

pub(crate) type CmdResult<T> = Result<T, String>;

pub(crate) fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
