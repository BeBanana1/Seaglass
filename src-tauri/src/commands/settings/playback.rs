//! Playback settings (#79, #492): read and saved in SQLite, applied live by the player.

use tauri::State;

use super::super::{AppState, CmdResult, err};
use crate::i18n::tr;
use crate::player::output::AudioDevices;
use crate::player::{Cmd, PlaybackSettings};

#[tauri::command]
pub async fn playback_settings(state: State<'_, AppState>) -> CmdResult<PlaybackSettings> {
    Ok(PlaybackSettings::load(state.store.as_deref()))
}

/// Quality currently lowered by the battery (#492), for Settings › Playback; `None` if the
/// battery changes nothing.
#[cfg(target_os = "android")]
#[tauri::command]
pub async fn quality_cap(
    state: State<'_, AppState>,
) -> CmdResult<Option<crate::player::QualityCap>> {
    let settings = PlaybackSettings::load(state.store.as_deref());
    Ok(crate::player::current_cap(&settings).await)
}

/// Saves then applies live. Deserializing `settings` validates the cache size: only the
/// values offered are accepted.
#[tauri::command]
pub async fn set_playback_settings(
    state: State<'_, AppState>,
    settings: PlaybackSettings,
) -> CmdResult<()> {
    let Some(store) = &state.store else {
        return Err(tr(
            "base SQLite indisponible, réglage non enregistré",
            "SQLite database unavailable, setting not saved",
        )
        .into());
    };
    settings.save(store).map_err(err)?;
    state
        .player
        .send(Cmd::SetPlaybackSettings(settings))
        .await
        .map_err(|_| crate::i18n::tr("lecteur arrêté", "player stopped").to_owned())
}

/// Output devices known to mpv (#79); the list then follows the `audio-devices` event
/// (device plugged in, default device changed).
#[tauri::command]
pub async fn audio_devices(state: State<'_, AppState>) -> CmdResult<AudioDevices> {
    let stopped = || tr("lecteur arrêté", "player stopped").to_owned();
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .player
        .send(Cmd::GetAudioDevices(tx))
        .await
        .map_err(|_| stopped())?;
    rx.await.map_err(|_| stopped())
}
