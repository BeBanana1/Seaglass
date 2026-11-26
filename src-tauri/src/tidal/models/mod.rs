//! Domain types exposed to the rest of the app. The UI sees them through `src/lib/ipc.ts`;
//! those deriving `ts_rs::TS` are generated into `src/lib/bindings/` (#44).

mod catalog;
mod info;
mod library;
mod stream;
mod track;

pub use catalog::*;
pub use info::*;
pub use library::*;
pub use stream::*;
pub use track::*;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub user_id: u64,
    pub country_code: String,
}
