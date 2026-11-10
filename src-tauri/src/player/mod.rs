//! Lecteur : un acteur unique possède la file et l'état ; tout passe par `Cmd`.

pub mod actor;
mod listen;
pub mod normalize;
pub mod output;
mod persist;
mod playback;
pub mod queue;
mod radio;
pub mod settings;
pub mod sleep;
pub mod source;
pub mod state;
mod upgrade;
pub mod volume;

pub use actor::{Cmd, spawn};
pub use persist::flush_blocking;
pub use queue::{QueuePart, Repeat};
pub use settings::{HiresUpgrade, PlaybackSettings};
pub use source::PlaySource;
pub use state::{PlayerState, QueueView};
pub use upgrade::is_better;
