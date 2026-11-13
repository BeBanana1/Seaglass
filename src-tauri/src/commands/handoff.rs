//! Ordinateur lié et transfert de la lecture (#414), côté téléphone : lier en collant
//! l'adresse de la télécommande du PC, « Reprendre ici », « Continuer sur le PC ».

use tauri::{AppHandle, State};

use super::{AppState, CmdResult};
use crate::handoff::{self, Link, LinkInfo};
use crate::i18n::tr;

async fn linked() -> CmdResult<Link> {
    handoff::load()
        .await?
        .ok_or_else(|| tr("aucun ordinateur lié", "no linked computer").to_owned())
}

/// Ordinateur lié, ou `null`.
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

/// Lie l'ordinateur dont `link` est l'adresse de la télécommande (avec son code après
/// `#`) : vérifié auprès du PC avant d'être gardé.
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
    // Chemin par le relais (#430), appris ici sur le réseau local.
    let route = link.fetch_relay().await.unwrap_or_else(|e| {
        tracing::info!("Ordinateur lié sans chemin par le relais ({e})");
        None
    });
    handoff::save_relay(route.clone()).await?;
    handoff::history::spawn(&app, None);
    Ok(link.with_relay(route).info())
}

/// Oublie l'ordinateur lié.
#[tauri::command]
pub async fn unlink_computer() -> CmdResult<()> {
    handoff::clear().await
}

/// « Reprendre ici » : la lecture de l'ordinateur lié passe sur cet appareil.
#[tauri::command]
pub async fn handoff_pull(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    handoff::pull(&linked().await?, &state.player).await?;
    handoff::history::spawn(&app, None);
    Ok(())
}

/// « Continuer sur l'ordinateur » : la lecture d'ici passe sur l'ordinateur lié.
#[tauri::command]
pub async fn handoff_push(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    handoff::push(&linked().await?, &state.player).await?;
    handoff::history::spawn(&app, None);
    Ok(())
}

/// Synchronise l'historique d'écoute avec l'ordinateur lié (#430) : `force` pour le
/// bouton de Réglages, sinon rien si la dernière synchro est toute récente (ouverture des
/// statistiques). Date de la dernière synchro réussie, `null` sans ordinateur lié.
#[tauri::command]
pub async fn sync_history(app: AppHandle, force: bool) -> CmdResult<Option<i64>> {
    handoff::history::sync_now(&app, force).await
}
