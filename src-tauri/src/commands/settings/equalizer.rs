//! Equalizer (#428): the profiles are saved with the playback settings
//! (`set_playback_settings`); here only the import of an AutoEQ profile, parsed on the
//! Rust side so that the UI has no rule of its own.

use super::super::CmdResult;
use crate::player::autoeq;
use crate::player::eq::EqProfile;

/// Parses the text of an AutoEQ "ParametricEQ.txt" (pasted, or read by the UI from a
/// chosen file). The returned profile has id 0 and an empty name, set by the UI.
#[tauri::command]
pub async fn parse_autoeq(text: String) -> CmdResult<EqProfile> {
    autoeq::parse(&text).map_err(|e| e.to_string())
}
