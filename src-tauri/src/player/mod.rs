//! Player: a single actor owns the queue and the state; everything goes through `Cmd`.

pub mod actor;
pub mod autoeq;
pub mod blind;
mod command;
mod dispatch;
pub mod eq;
mod exclusive;
pub mod handoff;
mod listen;
pub mod normalize;
pub mod output;
mod persist;
mod playback;
mod playlog;
mod profiles;
mod quality;
pub mod queue;
mod radio;
mod recover;
mod report;
pub mod settings;
pub mod sleep;
pub mod source;
pub mod state;
mod takeover;
mod upgrade;
pub mod volume;

pub use actor::{Cmd, spawn};
pub use exclusive::supported as exclusive_supported;
pub use handoff::Handoff;
#[cfg(desktop)]
pub use persist::flush_blocking;
#[cfg(target_os = "android")]
pub use quality::{QualityCap, current_cap};
pub use queue::{QueuePart, Repeat};
pub use report::start as start_play_log;
pub use settings::{HiresUpgrade, PlaybackSettings};
pub use source::PlaySource;
pub use state::{PlayerState, QueueView};
pub use upgrade::is_better;
