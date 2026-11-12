//! Sauvegarde des réglages dans un texte JSON (#352), copié et collé par le presse-papiers :
//! préférences d'interface et de lecture seulement. Ni jeton, ni session, ni file, ni
//! historique. Le périphérique de sortie et le mode exclusif, propres à la machine, ne sont
//! pas exportés.
//!
//! L'import reprend les clés une par une sur les réglages courants : chacune repasse par la
//! désérialisation qui valide déjà les commandes de réglage (thème, zoom, accents…), puis
//! par les mêmes contrôles (épingles, tris, accueil). Une clé invalide, inconnue ou propre
//! à l'autre plateforme est ignorée et citée dans le résumé.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tauri::{AppHandle, State};

use super::super::{AppState, CmdResult, err};
use super::home_layout::{HomeLayout, MAX_KEY_LEN, MAX_SECTIONS};
use super::{UiSettings, load, pins, sorts};
use crate::i18n::tr;
use crate::player::{Cmd, PlaybackSettings};

const FORMAT: &str = "seaglass-settings";
const VERSION: u32 = 1;
/// Taille maximale du texte collé : un export réel fait quelques kilo-octets.
const MAX_LEN: usize = 256 * 1024;

/// Clés de lecture liées à la machine : jamais exportées ni importées.
const PLAYBACK_LOCAL: [&str; 2] = ["audioDevice", "exclusive"];
/// Réglages du bureau seulement (Android : ignorés à l'import).
const UI_DESKTOP: [&str; 3] = ["closeAction", "discord", "zoom"];
const PLAYBACK_DESKTOP: [&str; 3] = ["cacheSize", "pauseOnUnplug", "resumeOnLaunch"];
/// Réglages d'Android seulement (bureau : ignorés à l'import).
const PLAYBACK_ANDROID: [&str; 3] = ["wifiQuality", "cellularQuality", "lowBatteryQuality"];

#[derive(Debug, Serialize, Deserialize)]
struct Backup {
    format: String,
    version: u32,
    #[serde(default)]
    ui: Option<Value>,
    #[serde(default)]
    playback: Option<Value>,
}

/// Clés reprises et ignorées, par section (`ui.theme`, `playback.autoRadio`…).
#[derive(Debug, Default, PartialEq, Serialize)]
pub struct ImportReport {
    pub applied: Vec<String>,
    pub ignored: Vec<String>,
}

fn export(ui: &UiSettings, playback: &PlaybackSettings) -> serde_json::Result<String> {
    let mut playback = serde_json::to_value(playback)?;
    if let Some(map) = playback.as_object_mut() {
        for key in PLAYBACK_LOCAL {
            map.remove(key);
        }
    }
    serde_json::to_string_pretty(&Backup {
        format: FORMAT.into(),
        version: VERSION,
        ui: Some(serde_json::to_value(ui)?),
        playback: Some(playback),
    })
}

fn invalid() -> String {
    tr(
        "ce texte n'est pas une sauvegarde de réglages de Seaglass",
        "this text is not a Seaglass settings backup",
    )
    .into()
}

/// Reprend les clés de `text` sur les réglages courants. Logique pure : `desktop` choisit
/// les clés propres à une plateforme.
fn import(
    text: &str,
    ui: &UiSettings,
    playback: &PlaybackSettings,
    desktop: bool,
) -> CmdResult<(UiSettings, PlaybackSettings, ImportReport)> {
    if text.len() > MAX_LEN {
        return Err(invalid());
    }
    let backup: Backup = serde_json::from_str(text.trim()).map_err(|_| invalid())?;
    if backup.format != FORMAT {
        return Err(invalid());
    }
    if backup.version > VERSION {
        return Err(tr(
            "sauvegarde faite par une version plus récente de Seaglass : mettez l'app à jour",
            "backup made by a newer version of Seaglass: update the app",
        )
        .into());
    }
    let mut report = ImportReport::default();

    let ui_skip: &[&str] = if desktop { &[] } else { &UI_DESKTOP };
    let mut ui = merge(ui, backup.ui.as_ref(), ui_skip, "ui", &mut report, ui_valid)?;
    ui.pins = pins::normalize(ui.pins).map_err(err)?;

    let mut playback_skip: Vec<&str> = PLAYBACK_LOCAL.to_vec();
    playback_skip.extend(if desktop {
        PLAYBACK_ANDROID
    } else {
        PLAYBACK_DESKTOP
    });
    let playback = merge(
        playback,
        backup.playback.as_ref(),
        &playback_skip,
        "playback",
        &mut report,
        |_, _| true,
    )?;
    Ok((ui, playback, report))
}

/// Contrôles des commandes de réglage que la désérialisation ne fait pas.
fn ui_valid(ui: &UiSettings, key: &str) -> bool {
    match key {
        "pins" => pins::normalize(ui.pins.clone()).is_ok(),
        "sorts" => sorts::are_valid(&ui.sorts),
        "home" => home_valid(&ui.home),
        _ => true,
    }
}

/// `HomeLayout::update` garde les sections absentes du flux du jour : l'ordre enregistré
/// peut dépasser `MAX_SECTIONS`, d'où une marge.
fn home_valid(home: &HomeLayout) -> bool {
    let key_ok = |k: &String| !k.is_empty() && k.chars().count() <= MAX_KEY_LEN;
    home.order.len() <= MAX_SECTIONS * 10
        && home.hidden.len() <= MAX_SECTIONS * 10
        && home.order.iter().all(key_ok)
        && home.hidden.iter().all(key_ok)
}

/// Pose les clés de `incoming` une à une sur `current` ; une clé que `T` refuse (ou que
/// `check` refuse) est ignorée, les autres restent.
fn merge<T: Serialize + DeserializeOwned>(
    current: &T,
    incoming: Option<&Value>,
    skip: &[&str],
    section: &str,
    report: &mut ImportReport,
    check: impl Fn(&T, &str) -> bool,
) -> CmdResult<T> {
    let Value::Object(mut base) = serde_json::to_value(current).map_err(err)? else {
        return Err(invalid());
    };
    if let Some(incoming) = incoming {
        let Value::Object(incoming) = incoming else {
            return Err(invalid());
        };
        for (key, value) in incoming {
            let name = format!("{section}.{key}");
            if skip.contains(&key.as_str()) || !base.contains_key(key) {
                report.ignored.push(name);
                continue;
            }
            let mut trial: Map<String, Value> = base.clone();
            trial.insert(key.clone(), value.clone());
            match serde_json::from_value::<T>(Value::Object(trial.clone())) {
                Ok(t) if check(&t, key) => {
                    base = trial;
                    report.applied.push(name);
                }
                _ => report.ignored.push(name),
            }
        }
    }
    serde_json::from_value(Value::Object(base)).map_err(err)
}

/// Réglages de cet appareil, en JSON à coller ailleurs.
#[tauri::command]
pub async fn export_settings(state: State<'_, AppState>) -> CmdResult<String> {
    let playback = PlaybackSettings::load(state.store.as_deref());
    export(&load(&state), &playback).map_err(err)
}

/// Importe une sauvegarde collée, l'enregistre et l'applique ; l'UI se recharge ensuite.
#[tauri::command]
pub async fn import_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
) -> CmdResult<ImportReport> {
    let Some(store) = &state.store else {
        return Err(tr(
            "base SQLite indisponible, réglage non enregistré",
            "SQLite database unavailable, setting not saved",
        )
        .into());
    };
    let current = PlaybackSettings::load(Some(store));
    let (ui, playback, report) = import(&text, &load(&state), &current, cfg!(desktop))?;
    super::store_and_apply(&app, &state, &ui)?;
    super::super::shortcuts::sync(&ui.pins);
    if playback != current {
        playback.save(store).map_err(err)?;
        state
            .player
            .send(Cmd::SetPlaybackSettings(playback))
            .await
            .map_err(|_| tr("lecteur arrêté", "player stopped").to_owned())?;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::super::Theme;
    use super::*;

    const UUID: &str = "1b7a5f3e-2c4d-4e6f-8a9b-0c1d2e3f4a5b";

    fn backup(ui: Value, playback: Value) -> String {
        serde_json::json!({ "format": FORMAT, "version": 1, "ui": ui, "playback": playback })
            .to_string()
    }

    #[test]
    fn export_leaves_out_machine_settings_and_round_trips() {
        let ui = UiSettings {
            theme: Theme::Studio,
            ..UiSettings::default()
        };
        let playback = PlaybackSettings {
            exclusive: true,
            auto_radio: true,
            ..PlaybackSettings::default()
        };
        let text = export(&ui, &playback).unwrap();
        assert!(!text.contains("exclusive") && !text.contains("audioDevice"));
        let (ui2, pb2, report) = import(
            &text,
            &UiSettings::default(),
            &PlaybackSettings::default(),
            true,
        )
        .unwrap();
        assert_eq!(ui2, ui);
        assert!(pb2.auto_radio && !pb2.exclusive);
        assert!(report.applied.contains(&"ui.theme".into()));
        assert!(report.ignored.contains(&"playback.wifiQuality".into()));
    }

    #[test]
    fn invalid_keys_are_ignored_one_by_one() {
        let text = backup(
            serde_json::json!({
                "theme": "neon",
                "motion": "reduced",
                "zoom": 125,
                "unknown": 1,
                "pins": [{ "kind": "playlist", "uuid": "nope", "title": "x" }],
                "sorts": [{ "list": UUID, "key": "title", "descending": true }],
            }),
            serde_json::json!({ "autoRadio": true, "audioDevice": "wasapi/x", "cacheSize": 999 }),
        );
        let ui = UiSettings::default();
        let (ui2, pb, report) = import(&text, &ui, &PlaybackSettings::default(), false).unwrap();
        assert_eq!(ui2.theme, Theme::Seaglass);
        assert_eq!(ui2.motion, super::super::Motion::Reduced);
        assert_eq!(ui2.zoom, ui.zoom, "zoom : bureau seulement");
        assert!(ui2.pins.is_empty());
        assert_eq!(ui2.sorts.len(), 1);
        assert!(pb.auto_radio && pb.audio_device.is_none());
        for key in [
            "ui.theme",
            "ui.zoom",
            "ui.unknown",
            "ui.pins",
            "playback.audioDevice",
            "playback.cacheSize",
        ] {
            assert!(report.ignored.contains(&key.into()), "{key}");
        }
    }

    #[test]
    fn rejects_other_text_and_newer_versions() {
        let d = (UiSettings::default(), PlaybackSettings::default());
        assert!(import("hello", &d.0, &d.1, true).is_err());
        assert!(import(r#"{"format":"other","version":1}"#, &d.0, &d.1, true).is_err());
        let newer = r#"{"format":"seaglass-settings","version":2}"#;
        assert!(import(newer, &d.0, &d.1, true).is_err());
        assert!(import(&"x".repeat(MAX_LEN + 1), &d.0, &d.1, true).is_err());
    }
}
