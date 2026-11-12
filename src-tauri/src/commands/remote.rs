//! Télécommande (#336) : allumer, éteindre, changer le code. Le choix et le code sont
//! retenus : allumée, elle repart au lancement suivant avec le même code (le QR code ne
//! se scanne qu'une fois).

use serde::{Deserialize, Serialize};
use tauri::State;
#[cfg(desktop)]
use tauri::{AppHandle, Manager};

use super::{AppState, CmdResult, err};
use crate::jam::remote::{self, RemoteState, RemoteStatus};
use crate::store::Store;

const KEY: &str = "remote";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct RemoteSettings {
    enabled: bool,
    /// Code de propriétaire ; tiré au premier allumage, gardé ensuite.
    code: Option<String>,
}

fn load(store: Option<&Store>) -> RemoteSettings {
    let settings = store
        .and_then(|s| match s.get_setting::<RemoteSettings>(KEY) {
            Ok(settings) => settings,
            Err(e) => {
                tracing::warn!("Télécommande : réglage illisible ({e})");
                None
            }
        })
        .unwrap_or_default();
    RemoteSettings {
        // Un code abîmé (édité à la main) est remplacé plutôt qu'accepté.
        code: settings.code.filter(|c| remote::is_code(c)),
        ..settings
    }
}

fn save(store: Option<&Store>, settings: &RemoteSettings) -> CmdResult<()> {
    if let Some(store) = store {
        store.set_setting(KEY, settings).map_err(err)?;
    }
    Ok(())
}

/// Télécommande en cours (adresse, code, QR code), ou `null` si elle est éteinte.
#[tauri::command]
pub async fn remote_status(remote: State<'_, RemoteState>) -> CmdResult<Option<RemoteStatus>> {
    Ok(remote.status())
}

/// Allume la télécommande (le code retenu, ou un nouveau) et retient le choix.
#[tauri::command]
pub async fn start_remote(
    state: State<'_, AppState>,
    remote: State<'_, RemoteState>,
) -> CmdResult<RemoteStatus> {
    let store = state.store.as_deref();
    let mut settings = load(store);
    let code = settings.code.clone().unwrap_or_else(remote::new_code);
    let status = remote
        .start(code.clone(), state.tidal.clone(), state.player.clone())
        .await?;
    settings.enabled = true;
    settings.code = Some(code);
    save(store, &settings)?;
    Ok(status)
}

/// Éteint la télécommande ; le code reste pour le prochain allumage.
#[tauri::command]
pub async fn stop_remote(
    state: State<'_, AppState>,
    remote: State<'_, RemoteState>,
) -> CmdResult<()> {
    remote.stop();
    let store = state.store.as_deref();
    let settings = RemoteSettings {
        enabled: false,
        ..load(store)
    };
    save(store, &settings)
}

/// Nouveau code : les téléphones appairés doivent scanner le nouveau QR code.
#[tauri::command]
pub async fn renew_remote_code(
    state: State<'_, AppState>,
    remote: State<'_, RemoteState>,
) -> CmdResult<Option<RemoteStatus>> {
    let store = state.store.as_deref();
    let code = remote::new_code();
    let settings = RemoteSettings {
        code: Some(code.clone()),
        ..load(store)
    };
    save(store, &settings)?;
    Ok(remote.set_code(code))
}

/// Au lancement : rallume la télécommande si elle l'était (bureau seulement).
#[cfg(desktop)]
pub fn restore(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let settings = load(state.store.as_deref());
        let (true, Some(code)) = (settings.enabled, settings.code) else {
            return;
        };
        let remote = app.state::<RemoteState>();
        if let Err(e) = remote
            .start(code, state.tidal.clone(), state.player.clone())
            .await
        {
            tracing::warn!("Télécommande : {e}");
        }
    });
}
