//! Commandes Tauri : valident l'entrée puis délèguent à `tidal` ou envoient un `Cmd`.

pub mod auth;
pub mod blocks;
pub mod catalog;
pub mod changelog;
pub mod cleanup;
pub mod collection;
pub mod drive;
#[cfg(desktop)]
pub mod export;
pub mod folders;
pub mod import;
pub mod jam;
pub mod later;
pub mod link;
pub mod logs;
#[cfg(desktop)]
pub mod mini_player;
pub mod play_history;
pub mod play_stats;
pub mod player;
pub mod project;
pub mod remote;
pub mod search_history;
pub mod settings;
pub mod shortcuts;
pub mod smart_lists;
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
