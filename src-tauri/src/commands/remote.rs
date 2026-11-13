//! Télécommande (#336) et transferts de lecture (#414) : allumer, éteindre, changer le
//! code. Les choix et le code sont retenus : le serveur repart au lancement suivant avec
//! le même code (le QR code ne se scanne qu'une fois, le téléphone reste lié).

use serde::{Deserialize, Serialize};
use tauri::State;
#[cfg(desktop)]
use tauri::{AppHandle, Manager};

use super::{AppState, CmdResult, err};
use crate::jam::remote::{self, Features, RemoteState, RemoteStatus};
use crate::store::Store;

const KEY: &str = "remote";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct RemoteSettings {
    /// Télécommande allumée.
    enabled: bool,
    /// Transferts de lecture acceptés depuis l'app Android (#414).
    #[serde(default)]
    handoff: bool,
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

impl RemoteSettings {
    fn features(&self) -> Features {
        Features {
            remote: self.enabled,
            handoff: self.handoff,
        }
    }
}

/// Applique et retient `settings` : le serveur sert ce qui est activé, s'arrête si rien
/// ne l'est. Le code retenu sert, ou un nouveau est tiré.
async fn apply(
    state: &AppState,
    remote: &RemoteState,
    mut settings: RemoteSettings,
) -> CmdResult<Option<RemoteStatus>> {
    let store = state.store.as_deref();
    let code = settings.code.clone().unwrap_or_else(remote::new_code);
    let status = remote
        .start(
            code.clone(),
            state.tidal.clone(),
            state.player.clone(),
            settings.features(),
        )
        .await?;
    settings.code = Some(code);
    save(store, &settings)?;
    Ok(status)
}

/// Allume la télécommande (le code retenu, ou un nouveau) et retient le choix.
#[tauri::command]
pub async fn start_remote(
    state: State<'_, AppState>,
    remote: State<'_, RemoteState>,
) -> CmdResult<Option<RemoteStatus>> {
    let settings = RemoteSettings {
        enabled: true,
        ..load(state.store.as_deref())
    };
    apply(&state, &remote, settings).await
}

/// Éteint la télécommande ; le serveur reste si les transferts sont acceptés, le code
/// reste pour le prochain allumage.
#[tauri::command]
pub async fn stop_remote(
    state: State<'_, AppState>,
    remote: State<'_, RemoteState>,
) -> CmdResult<Option<RemoteStatus>> {
    let settings = RemoteSettings {
        enabled: false,
        ..load(state.store.as_deref())
    };
    apply(&state, &remote, settings).await
}

/// « Accepter les transferts » (#414) : le téléphone lié peut reprendre la lecture du PC
/// ou la lui rendre, sans que la télécommande soit allumée.
#[tauri::command]
pub async fn set_handoff_accepted(
    enabled: bool,
    state: State<'_, AppState>,
    remote: State<'_, RemoteState>,
) -> CmdResult<Option<RemoteStatus>> {
    let settings = RemoteSettings {
        handoff: enabled,
        ..load(state.store.as_deref())
    };
    apply(&state, &remote, settings).await
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

/// Au lancement : rallume la télécommande et / ou les transferts (bureau seulement).
#[cfg(desktop)]
pub fn restore(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let settings = load(state.store.as_deref());
        let features = settings.features();
        let (true, Some(code)) = (features.any(), settings.code) else {
            return;
        };
        let remote = app.state::<RemoteState>();
        if let Err(e) = remote
            .start(code, state.tidal.clone(), state.player.clone(), features)
            .await
        {
            tracing::warn!("Télécommande : {e}");
        }
    });
}
