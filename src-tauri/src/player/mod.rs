//! Lecteur : un acteur unique possède la file et l'état ; tout passe par `Cmd`.

pub mod actor;
mod persist;
mod playback;
pub mod queue;
pub mod settings;
pub mod state;
mod upgrade;
pub mod volume;

pub use actor::{Cmd, spawn};
pub use queue::Repeat;
pub use settings::PlaybackSettings;
pub use state::{PlayerState, QueueView};
