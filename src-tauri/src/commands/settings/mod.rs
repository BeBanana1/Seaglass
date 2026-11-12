//! Réglages : interface et lecture dans SQLite (table `kv`, clés séparées), lancement au
//! démarrage lu et écrit directement dans le système (clé `Run` du registre).

pub mod accent;
pub mod home_layout;
pub mod pins;
pub mod sorts;
pub mod zoom;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
#[cfg(desktop)]
use tauri_plugin_autostart::ManagerExt;

use super::{AppState, CmdResult, err};
use crate::content::ContentFilter;
use crate::i18n::{self, Lang, LanguagePref, tr};
use crate::player::output::AudioDevices;
use crate::player::{Cmd, PlaybackSettings};
use crate::store::Store;
use accent::Accents;
use home_layout::{HomeLayout, SectionPref};
use pins::Pin;
use sorts::{ListSort, TrackSort};
use zoom::Zoom;

const KEY_UI: &str = "ui.settings";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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
    /// Fiche technique de livrée d'avion : bleu glacier, ardoise, jaune signal (#101).
    Aero,
    /// Carrosserie néo-rétro en aluminium satiné, feux en pixels ambre, clair (#175).
    Alloy,
    /// Alliage en version sombre, gunmetal (#175).
    #[serde(rename = "alloy-dark")]
    AlloyDark,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UiSettings {
    pub theme: Theme,
    /// Ordre et sections masquées de l'accueil (#21, #22).
    pub home: HomeLayout,
    /// Raccourcis de la barre latérale, dans l'ordre d'affichage (#10).
    pub pins: Vec<Pin>,
    /// Couleur d'accent choisie pour chaque thème (`#rrggbb`, #23, #250) ; thème absent :
    /// l'accent de son CSS.
    #[serde(deserialize_with = "accent::accents")]
    pub accents: Accents,
    /// Ancienne couleur unique (avant 0.4.104) : lue seulement, rangée dans `accents` sous
    /// `theme` par `migrate_accent`.
    #[serde(skip_serializing, deserialize_with = "accent::legacy")]
    accent: Option<String>,
    /// Langue de l'app (#7) ; `system` suit la langue d'affichage de Windows.
    pub language: LanguagePref,
    /// Tri mémorisé par playlist et pour les titres likés, le plus récent en tête (#93).
    pub sorts: Vec<ListSort>,
    /// Fermer la fenêtre principale : quitter, ou réduire dans la zone de notification (#83).
    pub close_action: CloseAction,
    /// Présence Discord (#215) : montrer le titre en cours à ses contacts. Désactivée par défaut.
    pub discord: bool,
    /// Taille de l'interface en pourcentage (#337), propre à chaque appareil.
    pub zoom: Zoom,
    /// Animations de l'interface (#351) : suivre le système, les réduire ou les garder.
    pub motion: Motion,
    /// Contenu masqué dans ce que le service propose (#368).
    pub content: ContentFilter,
}

/// `System` suit « réduire les animations » du système (`prefers-reduced-motion`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Motion {
    #[default]
    System,
    Reduced,
    Full,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CloseAction {
    #[default]
    Quit,
    Tray,
}

impl UiSettings {
    /// Range l'ancienne couleur unique sous le thème enregistré avec elle, sans écraser une
    /// couleur déjà choisie pour ce thème.
    pub(crate) fn migrate_accent(&mut self) {
        if let Some(color) = self.accent.take() {
            self.accents.entry(self.theme).or_insert(color);
        }
    }

    /// Reporte les réglages lus par le backend hors des commandes (fermeture de la fenêtre).
    pub(crate) fn apply_to_backend(&self) {
        self.content.apply();
        #[cfg(desktop)]
        crate::integrations::tray::set_close_to_tray(self.close_action == CloseAction::Tray);
        #[cfg(desktop)]
        crate::integrations::discord::set_enabled(self.discord);
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
    let mut settings = store
        .get_setting::<UiSettings>(KEY_UI)
        .unwrap_or_default()
        .unwrap_or_default();
    settings.migrate_accent();
    settings
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
pub async fn set_ui_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: UiSettings,
) -> CmdResult<()> {
    save(&state, &settings)?;
    i18n::set(settings.language.resolve());
    settings.apply_to_backend();
    #[cfg(desktop)]
    zoom::apply(&app, settings.zoom);
    #[cfg(not(desktop))]
    let _ = app;
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
    // Libellés traduits des raccourcis de l'icône (#335).
    super::shortcuts::sync(&settings.pins);
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

/// Affiche ou masque le bandeau « À la une » de l'accueil (#185).
#[tauri::command]
pub async fn set_home_featured(state: State<'_, AppState>, shown: bool) -> CmdResult<HomeLayout> {
    let mut settings = load(&state);
    settings.home.featured_hidden = !shown;
    save(&state, &settings)?;
    Ok(settings.home)
}

/// Revient à l'ordre du flux, sans section masquée, bandeau « À la une » compris.
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
    super::shortcuts::sync(&settings.pins);
    Ok(settings.pins)
}

/// Mémorise le tri d'une liste (uuid de playlist ou `likedTracks`), ou l'oublie avec
/// `null` ; renvoie les tris enregistrés, que l'UI reporte dans sa copie des réglages.
#[tauri::command]
pub async fn set_list_sort(
    state: State<'_, AppState>,
    list: String,
    sort: Option<TrackSort>,
) -> CmdResult<Vec<ListSort>> {
    let mut settings = load(&state);
    sorts::update(&mut settings.sorts, &list, sort).map_err(err)?;
    save(&state, &settings)?;
    Ok(settings.sorts)
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

/// Plateforme de l'app, pour que l'UI choisisse sa mise en page (tactile sur Android) et
/// masque les réglages sans objet. Décidée à la compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    #[cfg(desktop)]
    Desktop,
    #[cfg(mobile)]
    Android,
}

#[tauri::command]
pub async fn platform() -> CmdResult<Platform> {
    #[cfg(desktop)]
    return Ok(Platform::Desktop);
    #[cfg(mobile)]
    Ok(Platform::Android)
}

/// État réel du lancement au démarrage, lu dans le système à chaque appel : il peut avoir
/// été changé ailleurs (Gestionnaire des tâches), donc rien n'est gardé en base.
/// Toujours faux sur Android, qui n'a pas de lancement au démarrage.
#[tauri::command]
pub async fn autostart_enabled(app: AppHandle) -> CmdResult<bool> {
    #[cfg(desktop)]
    return app.autolaunch().is_enabled().map_err(err);
    #[cfg(mobile)]
    {
        let _ = app;
        Ok(false)
    }
}

/// Vrai si la présence Discord peut être proposée (#215) : bureau, identifiant de
/// l'application Discord compilé ou donné au lancement.
#[tauri::command]
pub async fn discord_available() -> CmdResult<bool> {
    #[cfg(desktop)]
    return Ok(crate::integrations::discord::client_id().is_some());
    #[cfg(mobile)]
    Ok(false)
}

/// Vrai si le mode exclusif peut être proposé (#245) : mpv sous Windows seulement (#17).
#[tauri::command]
pub async fn exclusive_available() -> CmdResult<bool> {
    Ok(crate::player::exclusive_supported())
}

/// Active ou désactive le lancement au démarrage, puis renvoie l'état relu.
#[cfg(desktop)]
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

/// Une entrée `Run` créée par une version antérieure à #83 n'a pas `--minimized` : réécrite au lancement si
/// elle est active (désactivée dans le Gestionnaire des tâches, elle n'est pas touchée).
#[cfg(desktop)]
pub(crate) fn refresh_autostart(app: &AppHandle) {
    let manager = app.autolaunch();
    if manager.is_enabled().unwrap_or(false)
        && let Err(e) = manager.enable()
    {
        tracing::warn!("lancement au démarrage non mis à jour : {e}");
    }
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
        let s: UiSettings = serde_json::from_str(r#"{"theme":"aero"}"#).unwrap();
        assert_eq!(s.theme, Theme::Aero);
        let s: UiSettings = serde_json::from_str(r#"{"theme":"alloy"}"#).unwrap();
        assert_eq!(s.theme, Theme::Alloy);
        let s: UiSettings = serde_json::from_str(r#"{"theme":"alloy-dark"}"#).unwrap();
        assert_eq!(s.theme, Theme::AlloyDark);
    }

    #[test]
    fn close_action_defaults_to_quit() {
        let s: UiSettings = serde_json::from_str(r#"{"theme":"studio"}"#).unwrap();
        assert_eq!(s.close_action, CloseAction::Quit);
        let s: UiSettings = serde_json::from_str(r#"{"closeAction":"tray"}"#).unwrap();
        assert_eq!(s.close_action, CloseAction::Tray);
        assert!(serde_json::from_str::<UiSettings>(r#"{"closeAction":"hide"}"#).is_err());
        assert_eq!(
            s.motion,
            Motion::System,
            "animations : système par défaut (#351)"
        );
        let s: UiSettings = serde_json::from_str(r#"{"motion":"reduced"}"#).unwrap();
        assert_eq!(s.motion, Motion::Reduced);
        assert!(serde_json::from_str::<UiSettings>(r#"{"motion":"none"}"#).is_err());
    }

    /// Les réglages enregistrés avant l'accueil personnalisable restent lisibles.
    #[test]
    fn home_layout_defaults_and_round_trips() {
        let s: UiSettings = serde_json::from_str(r#"{"theme":"atelier"}"#).unwrap();
        assert_eq!(s.home, HomeLayout::default());
        assert!(s.pins.is_empty());
        assert!(s.sorts.is_empty());
        assert!(s.accents.is_empty());
        let s: UiSettings =
            serde_json::from_str(r#"{"theme":"studio","home":{"order":["A","B"],"hidden":["B"]}}"#)
                .unwrap();
        assert_eq!(s.home.order, ["A", "B"]);
        assert!(s.home.hidden.contains("B"));
        let back: UiSettings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }
}
