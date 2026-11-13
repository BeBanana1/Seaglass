//! Ordinateur lié et transfert de la lecture (#414), côté téléphone : lier en collant
//! l'adresse de la télécommande du PC, « Reprendre ici », « Continuer sur le PC ».

use tauri::State;

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
pub async fn linked_computer() -> CmdResult<Option<LinkInfo>> {
    Ok(handoff::load().await?.map(|link| link.info()))
}

/// Lie l'ordinateur dont `link` est l'adresse de la télécommande (avec son code après
/// `#`) : vérifié auprès du PC avant d'être gardé.
#[tauri::command]
pub async fn link_computer(link: String) -> CmdResult<LinkInfo> {
    let link = Link::parse(&link).ok_or_else(|| {
        tr(
            "adresse non reconnue : copiez-la depuis Réglages › Application › Télécommande sur l'ordinateur",
            "address not recognized: copy it from Settings › App › Remote on the computer",
        )
        .to_owned()
    })?;
    link.check().await?;
    handoff::save(&link).await?;
    Ok(link.info())
}

/// Oublie l'ordinateur lié.
#[tauri::command]
pub async fn unlink_computer() -> CmdResult<()> {
    handoff::clear().await
}

/// « Reprendre ici » : la lecture de l'ordinateur lié passe sur cet appareil.
#[tauri::command]
pub async fn handoff_pull(state: State<'_, AppState>) -> CmdResult<()> {
    handoff::pull(&linked().await?, &state.player).await
}

/// « Continuer sur l'ordinateur » : la lecture d'ici passe sur l'ordinateur lié.
#[tauri::command]
pub async fn handoff_push(state: State<'_, AppState>) -> CmdResult<()> {
    handoff::push(&linked().await?, &state.player).await
}
