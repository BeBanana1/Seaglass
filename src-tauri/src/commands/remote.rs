//! Remote (#336) and playback handoff (#414): turn on, turn off, change the code, link a
//! phone (#62). The choices and the code are kept: the server starts again at the next
//! launch with the same code (the QR code is only scanned once, the phone stays linked).

use serde::{Deserialize, Serialize};
#[cfg(desktop)]
use tauri::Manager;
use tauri::{AppHandle, Emitter, State};

use super::{AppState, CmdResult, err};
use crate::i18n::tr;
use crate::jam::remote::{self, Features, RemoteState, RemoteStatus, TunnelSettings};
use crate::store::Store;

const KEY: &str = "remote";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct RemoteSettings {
    /// Remote turned on.
    enabled: bool,
    /// Playback handoffs accepted from the Android app (#414).
    #[serde(default)]
    handoff: bool,
    /// Owner code; drawn the first time it is turned on, kept afterwards.
    code: Option<String>,
    /// Identity on the relay and key shared with the phone (#430); drawn the first time
    /// it is turned on, the key changed with the code and at each "Link a phone" (#62).
    #[serde(default)]
    tunnel: Option<TunnelSettings>,
}

fn load(store: Option<&Store>) -> RemoteSettings {
    let settings = store
        .and_then(|s| match s.get_setting::<RemoteSettings>(KEY) {
            Ok(settings) => settings,
            Err(e) => {
                tracing::warn!("Remote: unreadable setting ({e})");
                None
            }
        })
        .unwrap_or_default();
    RemoteSettings {
        // A damaged code (edited by hand) is replaced rather than accepted.
        code: settings.code.filter(|c| remote::is_code(c)),
        tunnel: settings.tunnel.and_then(TunnelSettings::checked),
        ..settings
    }
}

fn save(store: Option<&Store>, settings: &RemoteSettings) -> CmdResult<()> {
    if let Some(store) = store {
        store.set_setting(KEY, settings).map_err(err)?;
    }
    Ok(())
}

/// Current remote (address, code, QR code), or `null` if it is off.
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

/// Listening history served to the linked phone (#430): the UI reloads the home page and
/// the stats when plays arrive.
fn history(app: &AppHandle, state: &AppState) -> Option<remote::History> {
    let app = app.clone();
    state.store.clone().map(|store| remote::History {
        store,
        on_added: Box::new(move |added| {
            let _ = app.emit(crate::handoff::history::EVENT, added);
        }),
    })
}

/// Applies and keeps `settings`: the server serves what is on, and stops if nothing is.
/// The kept code is used, or a new one is drawn.
async fn apply(
    app: &AppHandle,
    state: &AppState,
    remote: &RemoteState,
    mut settings: RemoteSettings,
) -> CmdResult<Option<RemoteStatus>> {
    let store = state.store.as_deref();
    let code = settings.code.clone().unwrap_or_else(remote::new_code);
    let tunnel = settings.tunnel.clone().unwrap_or_else(TunnelSettings::new);
    let status = remote
        .start(
            code.clone(),
            state.tidal.clone(),
            state.player.clone(),
            history(app, state),
            Some(tunnel.clone()),
            settings.features(),
        )
        .await?;
    settings.code = Some(code);
    settings.tunnel = Some(tunnel);
    save(store, &settings)?;
    Ok(status)
}

/// Turns the remote on (the kept code, or a new one) and keeps the choice.
#[tauri::command]
pub async fn start_remote(
    app: AppHandle,
    state: State<'_, AppState>,
    remote: State<'_, RemoteState>,
) -> CmdResult<Option<RemoteStatus>> {
    let settings = RemoteSettings {
        enabled: true,
        ..load(state.store.as_deref())
    };
    apply(&app, &state, &remote, settings).await
}

/// Turns the remote off; the server stays if handoffs are accepted, the code stays for
/// the next time it is turned on.
#[tauri::command]
pub async fn stop_remote(
    app: AppHandle,
    state: State<'_, AppState>,
    remote: State<'_, RemoteState>,
) -> CmdResult<Option<RemoteStatus>> {
    let settings = RemoteSettings {
        enabled: false,
        ..load(state.store.as_deref())
    };
    apply(&app, &state, &remote, settings).await
}

/// "Accept transfers" (#414): the linked phone can take the PC's playback or hand it
/// back, without the remote being on.
#[tauri::command]
pub async fn set_handoff_accepted(
    app: AppHandle,
    enabled: bool,
    state: State<'_, AppState>,
    remote: State<'_, RemoteState>,
) -> CmdResult<Option<RemoteStatus>> {
    let settings = RemoteSettings {
        handoff: enabled,
        ..load(state.store.as_deref())
    };
    apply(&app, &state, &remote, settings).await
}

/// The same tunnel identity with a new key: phones holding the old one no longer get
/// through the relay.
fn with_new_key(tunnel: Option<TunnelSettings>) -> Option<TunnelSettings> {
    tunnel.map(|t| TunnelSettings {
        key: TunnelSettings::new().key,
        ..t
    })
}

/// New code: paired phones must scan the new QR code.
#[tauri::command]
pub async fn renew_remote_code(
    state: State<'_, AppState>,
    remote: State<'_, RemoteState>,
) -> CmdResult<Option<RemoteStatus>> {
    let store = state.store.as_deref();
    let code = remote::new_code();
    let previous = load(store);
    // New tunnel key too (#430): an unlinked phone no longer gets through the relay.
    let tunnel = with_new_key(previous.tunnel.clone());
    let settings = RemoteSettings {
        code: Some(code.clone()),
        tunnel: tunnel.clone(),
        ..previous
    };
    save(store, &settings)?;
    Ok(remote.set_code(code, tunnel))
}

/// "Link a phone" (#62): a new tunnel key, then `remote::PAIRING_WINDOW` during which a
/// phone on the local network can learn the relay route with it. Phones linked before
/// keep the local network, and get the relay back by linking again during a window.
#[tauri::command]
pub async fn open_handoff_pairing(
    state: State<'_, AppState>,
    remote: State<'_, RemoteState>,
) -> CmdResult<Option<RemoteStatus>> {
    let store = state.store.as_deref();
    let previous = load(store);
    if !previous.handoff {
        return Err(tr(
            "activez d'abord « Accepter les transferts »",
            "turn on “Accept transfers” first",
        )
        .to_owned());
    }
    let tunnel = with_new_key(previous.tunnel.clone());
    let settings = RemoteSettings {
        tunnel: tunnel.clone(),
        ..previous
    };
    save(store, &settings)?;
    Ok(remote.open_pairing(tunnel))
}

/// At launch: turns the remote and / or handoffs back on (desktop only).
#[cfg(desktop)]
pub fn restore(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let mut settings = load(state.store.as_deref());
        let features = settings.features();
        let (true, Some(code)) = (features.any(), settings.code.clone()) else {
            return;
        };
        // Settings from before #430: identity on the relay drawn now.
        if settings.tunnel.is_none() {
            settings.tunnel = Some(TunnelSettings::new());
            if let Err(e) = save(state.store.as_deref(), &settings) {
                tracing::warn!("Remote: {e}");
            }
        }
        let remote = app.state::<RemoteState>();
        if let Err(e) = remote
            .start(
                code,
                state.tidal.clone(),
                state.player.clone(),
                history(&app, &state),
                settings.tunnel,
                features,
            )
            .await
        {
            tracing::warn!("Remote: {e}");
        }
    });
}
