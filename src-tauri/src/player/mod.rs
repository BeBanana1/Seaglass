//! Lecteur : un acteur unique possède la file et l'état ; tout passe par `Cmd`.

pub mod actor;
mod exclusive;
mod listen;
pub mod normalize;
pub mod output;
mod persist;
mod playback;
mod profiles;
mod quality;
pub mod queue;
mod radio;
mod recover;
pub mod settings;
pub mod sleep;
pub mod source;
pub mod state;
mod upgrade;
pub mod volume;

pub use actor::{Cmd, spawn};
pub use exclusive::supported as exclusive_supported;
#[cfg(desktop)]
pub use persist::flush_blocking;
pub use queue::{QueuePart, Repeat};
pub use settings::{HiresUpgrade, PlaybackSettings};
pub use source::PlaySource;
pub use state::{PlayerState, QueueView};
pub use upgrade::is_better;
