//! Mode conduite (#193) : un appui sur une tuile lit tout de suite la source épinglée, sans
//! passer par sa page. Les titres sont cherchés ici, pas dans l'UI.

use tauri::{AppHandle, State};

use super::settings::pins::{self, Pin};
use super::{AppState, CmdResult, err};
use crate::i18n::tr;
use crate::player::{Cmd, PlaySource};
use crate::tidal::models::{LaterItem, TrackId};

/// Plafond du contexte, comme `play_tracks`.
const MAX_CONTEXT_LEN: usize = 10_000;

/// Lit l'épingle `pin` depuis son début (ou mélangée avec `shuffle`).
#[tauri::command]
pub async fn play_pin(state: State<'_, AppState>, pin: Pin, shuffle: bool) -> CmdResult<()> {
    let pin = pins::normalize(vec![pin])
        .map_err(err)?
        .pop()
        .ok_or_else(|| tr("épingle vide", "empty pin").to_owned())?;
    let ids: Vec<u64> = match &pin {
        Pin::Playlist { uuid, .. } => state
            .tidal
            .playlist_since(uuid, None)
            .await
            .map_err(err)?
            .map(|page| page.tracks.iter().map(|e| e.track.id.0).collect())
            .unwrap_or_default(),
        Pin::Mix { id, .. } => {
            let page = state.tidal.mix(id).await.map_err(err)?;
            page.tracks.iter().map(|t| t.id.0).collect()
        }
        Pin::LikedTracks => {
            let list = state.tidal.favorite_tracks().await.map_err(err)?;
            list.items.iter().map(|e| e.track.id.0).collect()
        }
        Pin::Later => later_tracks(&state.tidal.save_for_later().await.map_err(err)?.items),
    };
    let ids = context(ids)?;
    state
        .player
        .send(Cmd::PlayTracks {
            ids,
            start: 0,
            shuffle,
            source: Some(source(pin)),
        })
        .await
        .map_err(|_| tr("lecteur arrêté", "player stopped").to_owned())
}

/// Titres de « Plus tard », dans l'ordre de la liste ; albums, artistes et playlists ignorés,
/// comme Lecture dans l'onglet de la bibliothèque.
fn later_tracks(items: &[LaterItem]) -> Vec<u64> {
    items
        .iter()
        .filter_map(|item| match item {
            LaterItem::Track(entry) => Some(entry.track.id.0),
            _ => None,
        })
        .collect()
}

fn context(ids: Vec<u64>) -> CmdResult<Vec<TrackId>> {
    if ids.is_empty() {
        return Err(tr("aucun titre à lire", "no tracks to play").into());
    }
    Ok(ids.into_iter().take(MAX_CONTEXT_LEN).map(TrackId).collect())
}

/// « Joué depuis » (#124) d'une épingle déjà validée.
fn source(pin: Pin) -> PlaySource {
    match pin {
        Pin::Playlist { uuid, title } => PlaySource::Playlist { uuid, title },
        Pin::Mix { id, title } => PlaySource::Mix { id, title },
        Pin::LikedTracks => PlaySource::LikedTracks,
        Pin::Later => PlaySource::Later,
    }
}

/// Vrai si le lien `seaglass://drive` ou le raccourci de l'app a demandé le mode conduite
/// depuis le dernier appel ; toujours faux sur le bureau. Lu au démarrage de l'UI et à
/// l'événement `open-drive`.
#[tauri::command]
pub async fn take_drive_request() -> CmdResult<bool> {
    Ok(crate::integrations::take_drive_request())
}

/// Garde l'écran allumé tant que le mode conduite est ouvert (Android ; rien sur le bureau).
#[tauri::command]
pub async fn keep_screen_on(app: AppHandle, on: bool) -> CmdResult<()> {
    #[cfg(target_os = "android")]
    {
        tauri_plugin_seaglass_mobile::native(&app)
            .call("keepScreenOn", serde_json::json!({ "on": on }))
            .await
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, on);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_follow_pins() {
        assert_eq!(
            source(Pin::Mix {
                id: "0abc".into(),
                title: "Mix".into()
            }),
            PlaySource::Mix {
                id: "0abc".into(),
                title: "Mix".into()
            }
        );
        assert_eq!(source(Pin::LikedTracks), PlaySource::LikedTracks);
        assert_eq!(source(Pin::Later), PlaySource::Later);
    }

    #[test]
    fn empty_context_is_refused_and_long_one_capped() {
        assert!(context(vec![]).is_err());
        let ids = context((0..MAX_CONTEXT_LEN as u64 + 5).collect()).unwrap();
        assert_eq!(ids.len(), MAX_CONTEXT_LEN);
        assert_eq!(ids[0], TrackId(0));
    }
}
