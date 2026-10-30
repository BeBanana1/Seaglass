//! Réglages : interface et lecture dans SQLite (table `kv`, clés séparées), lancement au
//! démarrage lu et écrit directement dans le système (clé `Run` du registre).

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

use super::{AppState, CmdResult, err};
use crate::player::{Cmd, PlaybackSettings};

const KEY_UI: &str = "ui.settings";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Verre dépoli sur fond de pochette, couleurs accordées à l'album.
    #[default]
    Seaglass,
    /// Console sombre et dense, chaîne audio au premier plan.
    Studio,
    /// Clair, éditorial, grande typographie.
    Atelier,
    /// Néons acides, chrome liquide et hologrammes sur fond noir.
    Acidwave,
    /// Paysage vectoriel onirique, aplats pastel et objets qui flottent.
    Surreal,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UiSettings {
    pub theme: Theme,
}

#[tauri::command]
pub async fn ui_settings(state: State<'_, AppState>) -> CmdResult<UiSettings> {
    let Some(store) = &state.store else {
        return Ok(UiSettings::default());
    };
    // Réglage illisible (ancienne version, valeur inconnue) : valeurs par défaut.
    Ok(store
        .get_setting::<UiSettings>(KEY_UI)
        .unwrap_or_default()
        .unwrap_or_default())
}

/// La désérialisation de `settings` valide le thème : une valeur inconnue est refusée.
#[tauri::command]
pub async fn set_ui_settings(state: State<'_, AppState>, settings: UiSettings) -> CmdResult<()> {
    match &state.store {
        Some(store) => store.set_setting(KEY_UI, &settings).map_err(err),
        None => Err("base SQLite indisponible, réglage non enregistré".into()),
    }
}

#[tauri::command]
pub async fn playback_settings(state: State<'_, AppState>) -> CmdResult<PlaybackSettings> {
    Ok(PlaybackSettings::load(state.store.as_deref()))
}

/// Enregistre puis applique à chaud. La désérialisation de `settings` valide la taille
/// du cache : seules les valeurs proposées sont acceptées.
#[tauri::command]
pub async fn set_playback_settings(
    state: State<'_, AppState>,
    settings: PlaybackSettings,
) -> CmdResult<()> {
    let Some(store) = &state.store else {
        return Err("base SQLite indisponible, réglage non enregistré".into());
    };
    settings.save(store).map_err(err)?;
    state
        .player
        .send(Cmd::SetCacheSize(settings.cache_size))
        .await
        .map_err(|_| "lecteur arrêté".to_owned())
}

/// État réel du lancement au démarrage, lu dans le système à chaque appel : il peut avoir
/// été changé ailleurs (Gestionnaire des tâches), donc rien n'est gardé en base.
#[tauri::command]
pub async fn autostart_enabled(app: AppHandle) -> CmdResult<bool> {
    app.autolaunch().is_enabled().map_err(err)
}

/// Active ou désactive le lancement au démarrage, puis renvoie l'état relu.
#[tauri::command]
pub async fn set_autostart(app: AppHandle, enabled: bool) -> CmdResult<bool> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(err)?;
    } else {
        manager.disable().map_err(err)?;
    }
    manager.is_enabled().map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_theme_is_rejected_and_missing_fields_default() {
        assert!(serde_json::from_str::<UiSettings>(r#"{"theme":"neon"}"#).is_err());
        let s: UiSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(s.theme, Theme::Seaglass);
        let s: UiSettings = serde_json::from_str(r#"{"theme":"studio"}"#).unwrap();
        assert_eq!(s.theme, Theme::Studio);
        let s: UiSettings = serde_json::from_str(r#"{"theme":"acidwave"}"#).unwrap();
        assert_eq!(s.theme, Theme::Acidwave);
        let s: UiSettings = serde_json::from_str(r#"{"theme":"surreal"}"#).unwrap();
        assert_eq!(s.theme, Theme::Surreal);
    }
}
