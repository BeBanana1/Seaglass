//! Jam (#25): start (online, or on the local network from the desktop), stop, state,
//! host controls (#288).

use tauri::{AppHandle, State};

use super::{AppState, CmdResult, err};
use crate::jam::relay_settings::{self, RelayChange, RelayView};
use crate::jam::{JamMode, JamRules, JamState, JamStatus};
use crate::store::Store;

/// Host rules, kept from one Jam to the next (#288).
const KEY_RULES: &str = "jam.rules";

fn load_rules(store: Option<&Store>) -> JamRules {
    store
        .and_then(|s| match s.get_setting::<JamRules>(KEY_RULES) {
            Ok(rules) => rules,
            Err(e) => {
                tracing::warn!("Jam: rules unreadable, using defaults ({e})");
                None
            }
        })
        .unwrap_or_default()
        .sanitized()
}

/// Current Jam (address, code, QR code, connection, guests, rules), or `null`.
#[tauri::command]
pub async fn jam_status(jam: State<'_, JamState>) -> CmdResult<Option<JamStatus>> {
    Ok(jam.status())
}

/// True if the app can open an online Jam (usable relay, #452).
#[tauri::command]
pub async fn jam_online_available() -> CmdResult<bool> {
    Ok(crate::jam::online_available())
}

/// Relay of the online Jam and of the handoffs (#452): chosen, compiled in, usable.
#[tauri::command]
pub async fn relay_settings() -> CmdResult<RelayView> {
    Ok(relay_settings::view())
}

/// Changes relay (address checked by `/healthz`), goes back to the compiled-in one or turns it off.
/// Applies to the next Jam, and to the handoff tunnel on its next opening.
#[tauri::command]
pub async fn set_relay_settings(
    state: State<'_, AppState>,
    change: RelayChange,
) -> CmdResult<RelayView> {
    relay_settings::update(state.store.as_deref(), change).await
}

/// Starts the Jam (or returns the one that is running): address and code to give to the guests.
/// Online, the address then arrives through the `jam-status` event.
#[tauri::command]
pub async fn start_jam(
    app: AppHandle,
    state: State<'_, AppState>,
    jam: State<'_, JamState>,
    mode: JamMode,
) -> CmdResult<JamStatus> {
    let rules = load_rules(state.store.as_deref());
    jam.start(mode, rules, app, state.tidal.clone(), state.player.clone())
        .await
}

#[tauri::command]
pub async fn stop_jam(jam: State<'_, JamState>) -> CmdResult<()> {
    jam.stop();
    Ok(())
}

/// Host rules (play / pause by guests, limit per guest, albums):
/// saved, then applied to the current Jam. State of the Jam, or `null` without a Jam.
#[tauri::command]
pub async fn set_jam_rules(
    state: State<'_, AppState>,
    jam: State<'_, JamState>,
    rules: JamRules,
) -> CmdResult<Option<JamStatus>> {
    let rules = rules.sanitized();
    if let Some(store) = &state.store {
        store.set_setting(KEY_RULES, &rules).map_err(err)?;
    }
    Ok(jam.set_rules(rules))
}

/// Suspends (`false`) or reopens guest additions, for the duration of the Jam (#288).
#[tauri::command]
pub async fn set_jam_adds_open(
    jam: State<'_, JamState>,
    open: bool,
) -> CmdResult<Option<JamStatus>> {
    Ok(jam.set_adds_open(open))
}

/// Kicks a guest (their pending tracks are removed) or readmits them (#288).
#[tauri::command]
pub async fn set_jam_guest_banned(
    jam: State<'_, JamState>,
    id: u32,
    banned: bool,
) -> CmdResult<Option<JamStatus>> {
    Ok(jam.set_guest_banned(id, banned).await)
}
