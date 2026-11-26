//! Settings: interface and playback in SQLite (table `kv`, separate keys). The playback
//! commands are in `playback.rs`, platform and launch at startup in `system.rs`.

pub mod accent;
pub mod backup;
pub mod eggs;
pub mod equalizer;
pub mod favorites;
pub mod global_shortcuts;
pub mod home_layout;
pub mod pins;
pub mod playback;
pub mod sorts;
pub mod system;
pub mod toggles;
pub mod zoom;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use super::{AppState, CmdResult, err};
use crate::content::ContentFilter;
use crate::i18n::{self, Lang, LanguagePref, tr};
use crate::store::Store;
use accent::Accents;
use eggs::EasterEggs;
use home_layout::{HomeLayout, SectionPref};
use pins::Pin;
use sorts::{ListSort, TrackSort};
use toggles::DefaultOn;
use zoom::Zoom;

const KEY_UI: &str = "ui.settings";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Frosted glass over the cover, colors tuned to the album.
    #[default]
    Seaglass,
    /// Dark and dense console, signal path up front.
    Studio,
    /// Light, editorial, large typography.
    Atelier,
    /// Acid neons, liquid chrome and holograms on black.
    Acidwave,
    /// Dreamy vector landscape, flat pastels and floating objects.
    Surreal,
    /// Hopeful, green future: light, deep greens, solar gold (#65).
    Solarpunk,
    /// Neon city and targeting interface: yellow, cyan, magenta, cut corners (#66).
    Cyberpunk,
    /// Soft tone-on-tone reliefs, light: raised controls, flat lists (#67).
    Neumorph,
    /// Neumorphism en version sombre, graphite (#67).
    #[serde(rename = "neumorph-dark")]
    NeumorphDark,
    /// Monochrome CRT screen, all monospace; the accent is the phosphor color (#68).
    Terminal,
    /// Risograph-printed zine: paper, two inks, halftone, misregistration (#69).
    Riso,
    /// Bauhaus poster: grid, primaries, geometric shapes (#70).
    Bauhaus,
    /// Aircraft livery spec sheet: glacier blue, slate, signal yellow (#101).
    Aero,
    /// Neo-retro bodywork in satin aluminum, amber pixel lights, light (#175).
    Alloy,
    /// Alliage en version sombre, gunmetal (#175).
    #[serde(rename = "alloy-dark")]
    AlloyDark,
    /// OLED black: pure black background, thin rules, no blur or decor; only the cover in
    /// color (#338).
    Midnight,
    /// Estampe japonaise : papier washi, encre indigo, cachet vermillon, vagues en aplats (#338).
    Estampe,
    /// Art deco: midnight blue, gold rules, corner fans, cartouches with cut corners (#338).
    Artdeco,
    /// 70s-80s hi-fi: walnut, burnt orange, amplifier faces, reels and VU meters (#338).
    Cassette,
    /// The movie theater: theater black, red velvet, ivory title cards (#339).
    Seance,
    /// Spaceship screens of 70s-80s sci-fi: amber, numbered blocks, indicator lights (#339).
    Onboard,
    /// Film noir: strict black and white, venetian blinds, typewriter (#339).
    Noir,
    /// Grand hotel: pastel, symmetrical comedy, dusty pink and burgundy, cartouches (#339).
    Hotel,
    /// Desert: sand, ochres, low horizon, a rare deep blue accent (#339).
    Desert,
    /// Cut-paper title sequence of the 50s-60s: black paper, orange and red cutouts (#339).
    Generique,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UiSettings {
    pub theme: Theme,
    /// Order and hidden sections of the home page (#21, #22).
    pub home: HomeLayout,
    /// Sidebar shortcuts, in display order (#10).
    pub pins: Vec<Pin>,
    /// Accent color chosen for each theme (`#rrggbb`, #23, #250); a theme not there: the accent
    /// of its CSS.
    #[serde(deserialize_with = "accent::accents")]
    pub accents: Accents,
    /// Former single color (before 0.4.104): only read, stored in `accents` under `theme` by
    /// `migrate_accent`.
    #[serde(skip_serializing, deserialize_with = "accent::legacy")]
    accent: Option<String>,
    /// Favorite themes (#390), family ids, at the top of the settings gallery.
    #[serde(deserialize_with = "favorites::favorites")]
    pub favorite_themes: Vec<Theme>,
    /// App language (#7); `system` follows Windows' display language.
    pub language: LanguagePref,
    /// Sort remembered per playlist and for the liked tracks, newest first (#93).
    pub sorts: Vec<ListSort>,
    /// Closing the main window: quit, or minimize to the notification area (#83).
    pub close_action: CloseAction,
    /// Discord presence (#215): show the current track to one's contacts. Off by default.
    pub discord: bool,
    /// Interface size in percent (#337), specific to each device.
    pub zoom: Zoom,
    /// Interface animations (#351): follow the system, reduce them or keep them.
    pub motion: Motion,
    /// Interface easter eggs (#399), on by default.
    pub easter_eggs: EasterEggs,
    /// Content hidden from what the service offers (#368).
    pub content: ContentFilter,
    /// Global keyboard shortcuts (#413, desktop), off by default.
    pub global_shortcuts: global_shortcuts::GlobalShortcuts,
    /// Periodic update check (#452), on by default. Without a built-in update address, the
    /// switch is not offered.
    pub auto_updates: DefaultOn,
    /// Jam (#452): off, it disappears from the UI (Settings, "Now playing").
    pub jam: DefaultOn,
    /// Notification of the followed artists' new releases (#435). Off by default.
    pub release_notifications: bool,
}

/// `System` follows the system's "reduce animations" (`prefers-reduced-motion`).
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
    /// Stores the former single color under the theme saved with it, without overwriting a
    /// color already chosen for that theme.
    pub(crate) fn migrate_accent(&mut self) {
        if let Some(color) = self.accent.take() {
            self.accents.entry(self.theme).or_insert(color);
        }
    }

    /// Passes on the settings the backend reads outside the commands (closing the window).
    pub(crate) fn apply_to_backend(&self) {
        self.content.apply();
        crate::servers::set_auto_updates(self.auto_updates.0);
        crate::jam::set_enabled(self.jam.0);
        crate::commands::feed_alerts::set_enabled(self.release_notifications);
        #[cfg(desktop)]
        crate::integrations::tray::set_close_to_tray(self.close_action == CloseAction::Tray);
        #[cfg(desktop)]
        crate::integrations::discord::set_enabled(self.discord);
    }
}

/// Saved settings; unreadable (older version, unknown value): defaults.
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

/// Deserializing `settings` validates the theme: an unknown value is refused.
#[tauri::command]
pub async fn set_ui_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: UiSettings,
) -> CmdResult<()> {
    store_and_apply(&app, &state, &settings)
}

/// Saves the interface settings and passes on those that act outside the UI (language of
/// the messages, closing the window, Discord, zoom). Also used by the import (#352).
fn store_and_apply(app: &AppHandle, state: &AppState, settings: &UiSettings) -> CmdResult<()> {
    save(state, settings)?;
    i18n::set(settings.language.resolve());
    // New release notifications turned on: Android 13+ asks for the permission.
    if settings.release_notifications && !crate::commands::feed_alerts::enabled() {
        crate::integrations::notify::request_permission(app);
    }
    settings.apply_to_backend();
    // Jam turned off: the running one stops.
    if !settings.jam.0 {
        app.state::<crate::jam::JamState>().stop();
    }
    #[cfg(desktop)]
    zoom::apply(app, settings.zoom);
    #[cfg(desktop)]
    crate::integrations::hotkeys::apply(app, &settings.global_shortcuts);
    #[cfg(not(desktop))]
    let _ = app;
    Ok(())
}

/// Global shortcuts (#413): whether they are possible here, and the actions whose
/// combination could not be registered (taken by another app).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalShortcutsStatus {
    available: bool,
    failed: Vec<global_shortcuts::GlobalAction>,
}

#[tauri::command]
pub async fn global_shortcuts_status(app: AppHandle) -> CmdResult<GlobalShortcutsStatus> {
    #[cfg(desktop)]
    return Ok(GlobalShortcutsStatus {
        available: crate::integrations::hotkeys::available(&app),
        failed: crate::integrations::hotkeys::failed(),
    });
    #[cfg(not(desktop))]
    {
        let _ = app;
        Ok(GlobalShortcutsStatus {
            available: false,
            failed: Vec::new(),
        })
    }
}

/// Effective language: the setting's, or the system's for `system`.
#[tauri::command]
pub async fn language() -> CmdResult<Lang> {
    Ok(i18n::current())
}

/// Saves the language setting, applies it to the backend's messages and to the `locale` of
/// TIDAL requests, and returns the effective language.
#[tauri::command]
pub async fn set_language(state: State<'_, AppState>, language: LanguagePref) -> CmdResult<Lang> {
    let mut settings = load(&state);
    settings.language = language;
    save(&state, &settings)?;
    let lang = language.resolve();
    i18n::set(lang);
    // Translated labels of the icon's shortcuts (#335).
    super::shortcuts::sync(&settings.pins);
    Ok(lang)
}

/// Saves the order and hiding of the sections shown on the home page (in screen order);
/// sections missing from the feed keep their setting. Returns the saved layout, which the UI
/// copies into its settings.
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

/// Shows or hides the home page's "Featured" banner (#185).
#[tauri::command]
pub async fn set_home_featured(state: State<'_, AppState>, shown: bool) -> CmdResult<HomeLayout> {
    let mut settings = load(&state);
    settings.home.featured_hidden = !shown;
    save(&state, &settings)?;
    Ok(settings.home)
}

/// Back to the feed's order, with no hidden section, "Featured" banner included.
#[tauri::command]
pub async fn reset_home_layout(state: State<'_, AppState>) -> CmdResult<HomeLayout> {
    let mut settings = load(&state);
    settings.home = HomeLayout::default();
    save(&state, &settings)?;
    Ok(settings.home)
}

/// Replaces the sidebar pins; returns the saved list (validated, without duplicates), which
/// the UI copies into its settings.
#[tauri::command]
pub async fn set_pins(state: State<'_, AppState>, pins: Vec<Pin>) -> CmdResult<Vec<Pin>> {
    let mut settings = load(&state);
    settings.pins = pins::normalize(pins).map_err(err)?;
    save(&state, &settings)?;
    super::shortcuts::sync(&settings.pins);
    Ok(settings.pins)
}

/// Remembers a list's sort (playlist uuid or `likedTracks`), or forgets it with `null`;
/// returns the saved sorts, which the UI copies into its settings.
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
        let s: UiSettings = serde_json::from_str(r#"{"theme":"midnight"}"#).unwrap();
        assert_eq!(s.theme, Theme::Midnight);
        let s: UiSettings = serde_json::from_str(r#"{"theme":"estampe"}"#).unwrap();
        assert_eq!(s.theme, Theme::Estampe);
        let s: UiSettings = serde_json::from_str(r#"{"theme":"artdeco"}"#).unwrap();
        assert_eq!(s.theme, Theme::Artdeco);
        let s: UiSettings = serde_json::from_str(r#"{"theme":"cassette"}"#).unwrap();
        assert_eq!(s.theme, Theme::Cassette);
        let s: UiSettings = serde_json::from_str(r#"{"theme":"seance"}"#).unwrap();
        assert_eq!(s.theme, Theme::Seance);
        let s: UiSettings = serde_json::from_str(r#"{"theme":"onboard"}"#).unwrap();
        assert_eq!(s.theme, Theme::Onboard);
        let s: UiSettings = serde_json::from_str(r#"{"theme":"noir"}"#).unwrap();
        assert_eq!(s.theme, Theme::Noir);
        let s: UiSettings = serde_json::from_str(r#"{"theme":"hotel"}"#).unwrap();
        assert_eq!(s.theme, Theme::Hotel);
        let s: UiSettings = serde_json::from_str(r#"{"theme":"desert"}"#).unwrap();
        assert_eq!(s.theme, Theme::Desert);
        let s: UiSettings = serde_json::from_str(r#"{"theme":"generique"}"#).unwrap();
        assert_eq!(s.theme, Theme::Generique);
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

    /// Settings saved before the customizable home page stay readable.
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
