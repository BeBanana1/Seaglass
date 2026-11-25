//! Remote (#336) and playback handoff (#414): turn on, turn off, change the code, link a
//! phone (#62). The choices and the code are kept (`stored`, credential store since #61):
//! the server starts again at the next launch with the same code (the QR code is only
//! scanned once, the phone stays linked).

mod stored;

#[cfg(desktop)]
use tauri::Manager;
use tauri::{AppHandle, Emitter, State};

use super::{AppState, CmdResult, err};
use crate::i18n::tr;
use crate::jam::remote::{self, RemoteState, RemoteStatus, TunnelSettings};
use stored::{Keyring, RemoteSettings};

/// Kept settings, read off the async threads (the credential store blocks).
async fn load(state: &AppState) -> CmdResult<RemoteSettings> {
    let store = state.store.clone();
    tokio::task::spawn_blocking(move || stored::load(store.as_deref(), &Keyring))
        .await
        .map_err(err)?
}

async fn save(state: &AppState, settings: RemoteSettings) -> CmdResult<()> {
    let store = state.store.clone();
    tokio::task::spawn_blocking(move || stored::save(store.as_deref(), &Keyring, &settings))
        .await
        .map_err(err)?
}

/// Current remote (address, code, QR code), or `null` if it is off.
#[tauri::command]
pub async fn remote_status(remote: State<'_, RemoteState>) -> CmdResult<Option<RemoteStatus>> {
    Ok(remote.status())
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
    save(state, settings).await?;
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
        ..load(&state).await?
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
        ..load(&state).await?
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
        ..load(&state).await?
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
    let code = remote::new_code();
    let previous = load(&state).await?;
    // New tunnel key too (#430): an unlinked phone no longer gets through the relay.
    let tunnel = with_new_key(previous.tunnel.clone());
    let settings = RemoteSettings {
        code: Some(code.clone()),
        tunnel: tunnel.clone(),
        ..previous
    };
    save(&state, settings).await?;
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
    let previous = load(&state).await?;
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
    save(&state, settings).await?;
    Ok(remote.open_pairing(tunnel))
}

/// At launch: turns the remote and / or handoffs back on (desktop only). Not if the
/// credential store cannot be read: the code would otherwise be replaced.
#[cfg(desktop)]
pub fn restore(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let mut settings = match load(&state).await {
            Ok(settings) => settings,
            Err(e) => {
                tracing::warn!("Remote: not restored ({e})");
                return;
            }
        };
        let features = settings.features();
        let (true, Some(code)) = (features.any(), settings.code.clone()) else {
            return;
        };
        // Settings from before #430: identity on the relay drawn now.
        if settings.tunnel.is_none() {
            settings.tunnel = Some(TunnelSettings::new());
            if let Err(e) = save(&state, settings.clone()).await {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_key_keeps_the_tunnel_identity() {
        let before = TunnelSettings::new();
        let after = with_new_key(Some(before.clone())).unwrap();
        assert_eq!((&after.id, &after.secret), (&before.id, &before.secret));
        assert_ne!(after.key, before.key);
        assert_eq!(after.clone().checked(), Some(after));
        assert_eq!(with_new_key(None), None);
    }
}
