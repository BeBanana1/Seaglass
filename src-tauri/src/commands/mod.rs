//! Commandes Tauri : valident l'entrée puis délèguent à `tidal` ou envoient un `Cmd`.

pub mod auth;
pub mod catalog;
pub mod collection;
pub mod later;
pub mod link;
pub mod play_history;
pub mod player;
pub mod project;
pub mod search_history;
pub mod settings;
pub mod snapshots;
#[cfg(desktop)]
pub mod window;

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::player::Cmd;
use crate::store::Store;
use crate::tidal::TidalClient;

pub struct AppState {
    pub tidal: Arc<TidalClient>,
    pub player: mpsc::Sender<Cmd>,
    pub store: Option<Arc<Store>>,
}

pub(crate) type CmdResult<T> = Result<T, String>;

pub(crate) fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
