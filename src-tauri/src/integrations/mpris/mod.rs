//! Linux media controls (#312): an MPRIS server, the equivalent of the Windows SMTC (media keys,
//! desktop applet, `playerctl`, KDE Connect).

pub mod logic;
#[cfg(target_os = "linux")]
mod server;

#[cfg(target_os = "linux")]
pub use server::{MprisHandle, start};
