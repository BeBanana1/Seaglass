//! Contrôles média de Linux (#312) : serveur MPRIS, l'équivalent du SMTC de Windows
//! (touches média, applet du bureau, `playerctl`, KDE Connect).

pub mod logic;
#[cfg(target_os = "linux")]
mod server;

#[cfg(target_os = "linux")]
pub use server::{MprisHandle, start};
