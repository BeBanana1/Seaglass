//! Réglages : interface et lecture dans SQLite (table `kv`, clés séparées), lancement au
//! démarrage lu et écrit directement dans le système (clé `Run` du registre).

pub mod home_layout;
pub mod pins;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

use super::{AppState, CmdResult, err};
use crate::i18n::{self, Lang, LanguagePref, tr};
use crate::player::{Cmd, PlaybackSettings};
use crate::store::Store;
use home_layout::{HomeLayout, SectionPref};
use pins::Pin;

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
    /// Futur optimiste et végétal : clair, verts profonds, or solaire (#65).
    Solarpunk,
    /// Ville néon et interface de visée : jaune, cyan, magenta, angles coupés (#66).
    Cyberpunk,
    /// Reliefs doux ton sur ton, clair : commandes en relief, listes à plat (#67).
    Neumorph,
    /// Neumorphism en version sombre, graphite (#67).
    #[serde(rename = "neumorph-dark")]
    NeumorphDark,
    /// Écran cathodique monochrome, tout en mono ; l'accent est la couleur du phosphore (#68).
    Terminal,
    /// Fanzine imprimé en risographie : papier, deux encres, trame, décalage de repérage (#69).
    Riso,
    /// Affiche Bauhaus : grille, primaires, formes géométriques (#70).
    Bauhaus,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UiSettings {
    pub theme: Theme,
    /// Ordre et sections masquées de l'accueil (#21, #22).
    pub home: HomeLayout,
    /// Raccourcis de la barre latérale, dans l'ordre d'affichage (#10).
    pub pins: Vec<Pin>,
    /// Couleur d'accent choisie (`#rrggbb`, #23) ; absente : celle du thème.
    #[serde(deserialize_with = "accent_color")]
    pub accent: Option<String>,
    /// Langue de l'app (#7) ; `system` suit la langue d'affichage de Windows.
    pub language: LanguagePref,
}

/// N'accepte que `#rrggbb` (mis en minuscules) : la valeur part telle quelle dans le CSS.
fn accent_color<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let Some(color) = Option::<String>::deserialize(d)? else {
        return Ok(None);
    };
    let hex = color.strip_prefix('#').unwrap_or_default();
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(Some(color.to_ascii_lowercase()))
    } else {
        Err(serde::de::Error::custom(tr(
            "couleur d'accent invalide (attendu #rrggbb)",
            "invalid accent color (expected #rrggbb)",
        )))
    }
}

/// Réglages enregistrés ; illisibles (ancienne version, valeur inconnue) : valeurs par défaut.
pub(crate) fn load(state: &AppState) -> UiSettings {
    load_from(state.store.as_deref())
}

pub(crate) fn load_from(store: Option<&Store>) -> UiSettings {
    let Some(store) = store else {
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
        None => Err(tr(
            "base SQLite indisponible, réglage non enregistré",
            "SQLite database unavailable, setting not saved",
        )
        .into()),
    }
}

#[tauri::command]
pub async fn ui_settings(state: State<'_, AppState>) -> CmdResult<UiSettings> {
    Ok(load(&state))
}

/// La désérialisation de `settings` valide le thème : une valeur inconnue est refusée.
#[tauri::command]
pub async fn set_ui_settings(state: State<'_, AppState>, settings: UiSettings) -> CmdResult<()> {
    save(&state, &settings)?;
    i18n::set(settings.language.resolve());
    Ok(())
}

/// Langue effective : celle du réglage, ou celle du système pour `system`.
#[tauri::command]
pub async fn language() -> CmdResult<Lang> {
    Ok(i18n::current())
}

/// Enregistre le réglage de langue, l'applique aux messages du backend et à la `locale`
/// des requêtes TIDAL, et renvoie la langue effective.
#[tauri::command]
pub async fn set_language(state: State<'_, AppState>, language: LanguagePref) -> CmdResult<Lang> {
    let mut settings = load(&state);
    settings.language = language;
    save(&state, &settings)?;
    let lang = language.resolve();
    i18n::set(lang);
    Ok(lang)
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

/// Remplace les épingles de la barre latérale ; renvoie la liste enregistrée (validée,
/// sans doublons), que l'UI reporte dans sa copie des réglages.
#[tauri::command]
pub async fn set_pins(state: State<'_, AppState>, pins: Vec<Pin>) -> CmdResult<Vec<Pin>> {
    let mut settings = load(&state);
    settings.pins = pins::normalize(pins).map_err(err)?;
    save(&state, &settings)?;
    Ok(settings.pins)
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
        let s: UiSettings = serde_json::from_str(r#"{"theme":"solarpunk"}"#).unwrap();
        assert_eq!(s.theme, Theme::Solarpunk);
        let s: UiSettings = serde_json::from_str(r#"{"theme":"cyberpunk"}"#).unwrap();
        assert_eq!(s.theme, Theme::Cyberpunk);
        let s: UiSettings = serde_json::from_str(r#"{"theme":"neumorph"}"#).unwrap();
        assert_eq!(s.theme, Theme::Neumorph);
        let s: UiSettings = serde_json::from_str(r#"{"theme":"neumorph-dark"}"#).unwrap();
        assert_eq!(s.theme, Theme::NeumorphDark);
        let s: UiSettings = serde_json::from_str(r#"{"theme":"terminal"}"#).unwrap();
        assert_eq!(s.theme, Theme::Terminal);
        let s: UiSettings = serde_json::from_str(r#"{"theme":"riso"}"#).unwrap();
        assert_eq!(s.theme, Theme::Riso);
        let s: UiSettings = serde_json::from_str(r#"{"theme":"bauhaus"}"#).unwrap();
        assert_eq!(s.theme, Theme::Bauhaus);
    }

    #[test]
    fn accent_must_be_a_hex_color() {
        let s: UiSettings = serde_json::from_str(r##"{"accent":"#FF5C8A"}"##).unwrap();
        assert_eq!(s.accent.as_deref(), Some("#ff5c8a"));
        let s: UiSettings = serde_json::from_str(r#"{"accent":null}"#).unwrap();
        assert_eq!(s.accent, None);
        for bad in [
            r#""red""#,
            r##""#fff""##,
            r##""#12345g""##,
            r#""ff5c8a""#,
            r##""#ff5c8a;color:red""##,
            "1",
        ] {
            let json = format!(r#"{{"accent":{bad}}}"#);
            assert!(serde_json::from_str::<UiSettings>(&json).is_err(), "{bad}");
        }
    }

    /// Les réglages enregistrés avant l'accueil personnalisable restent lisibles.
    #[test]
    fn home_layout_defaults_and_round_trips() {
        let s: UiSettings = serde_json::from_str(r#"{"theme":"atelier"}"#).unwrap();
        assert_eq!(s.home, HomeLayout::default());
        assert!(s.pins.is_empty());
        assert_eq!(s.accent, None);
        let s: UiSettings =
            serde_json::from_str(r#"{"theme":"studio","home":{"order":["A","B"],"hidden":["B"]}}"#)
                .unwrap();
        assert_eq!(s.home.order, ["A", "B"]);
        assert!(s.home.hidden.contains("B"));
        let back: UiSettings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }
}
