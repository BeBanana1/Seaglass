//! Lecteur : un acteur unique possède l'état ; tout passe par `Cmd`.
//!
//! Phase 1 : lecture d'un titre à la fois. File d'attente, gapless et persistance
//! arrivent en phase 2 (`queue.rs`).

pub mod actor;
pub mod state;

pub use actor::{Cmd, spawn};
pub use state::PlayerState;
