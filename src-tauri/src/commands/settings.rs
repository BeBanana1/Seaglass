//! Réglages d'interface, stockés dans SQLite (table `kv`).

use serde::{Deserialize, Serialize};
use tauri::State;

use super::{AppState, CmdResult, err};
use crate::home_layout::{HomeLayout, SectionPref};

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
    /// Ordre et sections masquées de l'accueil (#21, #22).
    pub home: HomeLayout,
}

/// Réglages enregistrés ; illisibles (ancienne version, valeur inconnue) : valeurs par défaut.
pub(crate) fn load(state: &AppState) -> UiSettings {
    let Some(store) = &state.store else {
        return UiSettings::default();
    };
    store
        .get_setting::<UiSettings>(KEY_UI)
        .unwrap_or_default()
        .unwrap_or_default()
}

fn save(state: &AppState, settings: &UiSettings) -> CmdResult<()> {
    match &state.store {
        Some(store) => store.set_setting(KEY_UI, settings).map_err(err),
        None => Err("base SQLite indisponible, réglage non enregistré".into()),
    }
}

#[tauri::command]
pub async fn ui_settings(state: State<'_, AppState>) -> CmdResult<UiSettings> {
    Ok(load(&state))
}

/// La désérialisation de `settings` valide le thème : une valeur inconnue est refusée.
#[tauri::command]
pub async fn set_ui_settings(state: State<'_, AppState>, settings: UiSettings) -> CmdResult<()> {
    save(&state, &settings)
}

/// Enregistre l'ordre et le masquage des sections affichées sur l'accueil (dans l'ordre de
/// l'écran) ; les sections absentes du flux gardent leur réglage. Renvoie la disposition
/// enregistrée, que l'UI reporte dans sa copie des réglages.
#[tauri::command]
pub async fn set_home_layout(
    state: State<'_, AppState>,
    sections: Vec<SectionPref>,
) -> CmdResult<HomeLayout> {
    let mut settings = load(&state);
    settings.home.update(&sections).map_err(err)?;
    save(&state, &settings)?;
    Ok(settings.home)
}

/// Revient à l'ordre du flux, sans section masquée.
#[tauri::command]
pub async fn reset_home_layout(state: State<'_, AppState>) -> CmdResult<HomeLayout> {
    let mut settings = load(&state);
    settings.home = HomeLayout::default();
    save(&state, &settings)?;
    Ok(settings.home)
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

    /// Les réglages enregistrés avant l'accueil personnalisable restent lisibles.
    #[test]
    fn home_layout_defaults_and_round_trips() {
        let s: UiSettings = serde_json::from_str(r#"{"theme":"atelier"}"#).unwrap();
        assert_eq!(s.home, HomeLayout::default());
        let s: UiSettings =
            serde_json::from_str(r#"{"theme":"studio","home":{"order":["A","B"],"hidden":["B"]}}"#)
                .unwrap();
        assert_eq!(s.home.order, ["A", "B"]);
        assert!(s.home.hidden.contains("B"));
        let back: UiSettings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }
}
