//! Lecteur : un acteur unique possède la file et l'état ; tout passe par `Cmd`.

pub mod actor;
mod persist;
mod playback;
pub mod queue;
pub mod state;

pub use actor::{Cmd, spawn};
pub use queue::Repeat;
pub use state::{PlayerState, QueueView};
