//! Linked computer and playback handoff (#414), phone side: link by pasting
//! the address of the PC's remote control, "Resume here", "Continue on the PC".

use tauri::{AppHandle, State};

use super::{AppState, CmdResult};
use crate::handoff::{self, Link, LinkInfo};
use crate::i18n::tr;

async fn linked() -> CmdResult<Link> {
    handoff::load()
        .await?
        .ok_or_else(|| tr("aucun ordinateur lié", "no linked computer").to_owned())
}

/// Linked computer, or `null`.
#[tauri::command]
pub async fn linked_computer(state: State<'_, AppState>) -> CmdResult<Option<LinkInfo>> {
    let Some(link) = handoff::load().await? else {
        return Ok(None);
    };
    let synced_at = state
        .store
        .as_ref()
        .and_then(|s| s.sync_state().ok())
        .and_then(|s| s.synced_at);
    Ok(Some(LinkInfo {
        synced_at,
        ..link.info()
    }))
}

/// Links the computer whose remote control address is `link` (with its code after
/// `#`): verified with the PC before being kept.
#[tauri::command]
pub async fn link_computer(app: AppHandle, link: String) -> CmdResult<LinkInfo> {
    let link = Link::parse(&link).ok_or_else(|| {
        tr(
            "adresse non reconnue : copiez-la depuis Réglages › Application › Télécommande sur l'ordinateur",
            "address not recognized: copy it from Settings › App › Remote on the computer",
        )
        .to_owned()
    })?;
    link.check().await?;
    handoff::save(&link).await?;
    // Path through the relay (#430), learned here on the local network.
    let route = link.fetch_relay().await.unwrap_or_else(|e| {
        tracing::info!("Linked computer without a path through the relay ({e})");
        None
    });
    handoff::save_relay(route.clone()).await?;
    handoff::history::spawn(&app, None);
    Ok(link.with_relay(route).info())
}

/// Forgets the linked computer.
#[tauri::command]
pub async fn unlink_computer() -> CmdResult<()> {
    handoff::clear().await
}

/// "Resume here": the linked computer's playback moves to this device.
#[tauri::command]
pub async fn handoff_pull(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    handoff::pull(&linked().await?, &state.player).await?;
    handoff::history::spawn(&app, None);
    Ok(())
}

/// "Continue on the computer": playback from here moves to the linked computer.
#[tauri::command]
pub async fn handoff_push(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    handoff::push(&linked().await?, &state.player).await?;
    handoff::history::spawn(&app, None);
    Ok(())
}

/// Syncs the listening history with the linked computer (#430): `force` for the
/// Settings button, otherwise nothing if the last sync is very recent (opening the
/// statistics). Date of the last successful sync, `null` without a linked computer.
#[tauri::command]
pub async fn sync_history(app: AppHandle, force: bool) -> CmdResult<Option<i64>> {
    handoff::history::sync_now(&app, force).await
}
