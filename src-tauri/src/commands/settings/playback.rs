//! Réglages de lecture (#79, #492) : lus et enregistrés dans SQLite, appliqués à chaud par
//! le lecteur.

use tauri::State;

use super::super::{AppState, CmdResult, err};
use crate::i18n::tr;
use crate::player::output::AudioDevices;
use crate::player::{Cmd, PlaybackSettings};

#[tauri::command]
pub async fn playback_settings(state: State<'_, AppState>) -> CmdResult<PlaybackSettings> {
    Ok(PlaybackSettings::load(state.store.as_deref()))
}

/// Qualité baissée par la batterie en ce moment (#492), pour Réglages › Lecture ; `None`
/// si la batterie ne change rien.
#[cfg(target_os = "android")]
#[tauri::command]
pub async fn quality_cap(
    state: State<'_, AppState>,
) -> CmdResult<Option<crate::player::QualityCap>> {
    let settings = PlaybackSettings::load(state.store.as_deref());
    Ok(crate::player::current_cap(&settings).await)
}

/// Enregistre puis applique à chaud. La désérialisation de `settings` valide la taille
/// du cache : seules les valeurs proposées sont acceptées.
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

/// Périphériques de sortie connus de mpv (#79) ; la liste suit ensuite l'événement
/// `audio-devices` (branchement, changement du périphérique par défaut).
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
