//! Playlists intelligentes locales (#383) : listes fixes calculées sur l'historique
//! d'écoute de cet appareil (`store::smart`) et les titres likés, sans rien écrire chez
//! TIDAL. Le résultat passe par les mêmes filtres que ce que le service propose : titres
//! indisponibles, filtre de contenu (explicite, IA) et titres ou artistes bloqués.

use std::collections::HashSet;
use std::sync::Arc;

use tauri::State;

use super::play_history::now_ms;
use super::play_stats::{blocking, fetch};
use super::{AppState, CmdResult, err};
use crate::content::filtered;
use crate::store::{CountedTrack, SmartList, Store};
use crate::tidal::TidalClient;
use crate::tidal::models::{Track, TrackEntry, TrackId};

const DAY_MS: i64 = 24 * 3600 * 1000;
/// « Mes 50 du mois ».
const TOP_DAYS: i64 = 30;
const TOP_LIMIT: usize = 50;
/// « En boucle » : au moins `REPEAT_MIN` écoutes sur `REPEAT_DAYS` jours.
const REPEAT_DAYS: i64 = 7;
const REPEAT_MIN: i64 = 3;
/// « Oubliés » : au moins `FORGOTTEN_MIN` écoutes, aucune depuis `FORGOTTEN_DAYS` jours.
const FORGOTTEN_DAYS: i64 = 180;
const FORGOTTEN_MIN: i64 = 5;
const LIMIT: usize = 50;
/// « Likés jamais écoutés ».
const NEVER_LIMIT: usize = 100;
/// Marge de candidats demandés en plus, pour remplir la liste malgré les titres écartés.
const SPARE: usize = 2;
/// Titres vérifiés en parallèle.
const CHECK_PARALLEL: usize = 6;

/// Titres de la liste, dans son ordre.
#[tauri::command]
pub async fn smart_list(state: State<'_, AppState>, list: SmartList) -> CmdResult<Vec<Track>> {
    let Some(store) = state.store.clone() else {
        return Ok(Vec::new());
    };
    smart_tracks(&state.tidal, &store, list).await
}

pub(crate) async fn smart_tracks(
    tidal: &Arc<TidalClient>,
    store: &Arc<Store>,
    list: SmartList,
) -> CmdResult<Vec<Track>> {
    let now = now_ms();
    let (counted, limit) = match list {
        SmartList::NeverPlayed => return never_played(tidal, store).await,
        SmartList::TopMonth => {
            let since = now - TOP_DAYS * DAY_MS;
            let rows = blocking(store, move |s| s.top_tracks(since, 1, TOP_LIMIT * SPARE));
            (rows.await?, TOP_LIMIT)
        }
        SmartList::OnRepeat => {
            let since = now - REPEAT_DAYS * DAY_MS;
            let rows = blocking(store, move |s| {
                s.top_tracks(since, REPEAT_MIN, LIMIT * SPARE)
            });
            (rows.await?, LIMIT)
        }
        SmartList::Forgotten => {
            let before = now - FORGOTTEN_DAYS * DAY_MS;
            let rows = blocking(store, move |s| {
                s.forgotten_tracks(before, FORGOTTEN_MIN, LIMIT * SPARE)
            });
            (rows.await?, LIMIT)
        }
    };
    let ids: Vec<TrackId> = counted.iter().map(|(id, _, _)| *id).collect();
    // Avant les métadonnées : la vérification met les titres dans le cache du catalogue.
    let unavailable = unavailable(tidal, &ids).await;
    let missing: Vec<TrackId> = counted
        .iter()
        .filter(|(id, track, _)| track.is_none() && !unavailable.contains(id))
        .map(|(id, _, _)| *id)
        .collect();
    let fetched = fetch(tidal, &missing).await;
    if !fetched.is_empty() {
        let save = fetched.clone();
        if let Err(e) = blocking(store, move |s| s.save_played_tracks(&save)).await {
            tracing::warn!("titres de l'historique non complétés : {e}");
        }
    }
    let tracks = assemble(counted, &fetched, &unavailable);
    Ok(finish(tidal, tracks, limit).await)
}

/// Titres likés absents de l'historique, du like le plus récent au plus ancien.
async fn never_played(tidal: &Arc<TidalClient>, store: &Arc<Store>) -> CmdResult<Vec<Track>> {
    let liked = tidal.favorite_tracks().await.map_err(err)?;
    let played = blocking(store, |s| s.played_track_ids()).await?;
    // Ce qui est masqué ne compte pas dans les candidats à vérifier.
    let candidates: Vec<Track> = filtered(unplayed(liked.items, &played));
    let candidates: Vec<Track> = candidates.into_iter().take(NEVER_LIMIT * SPARE).collect();
    let ids: Vec<TrackId> = candidates.iter().map(|t| t.id).collect();
    // Déjà connue par la lecture des favoris : pas de requête en pratique.
    let unavailable = unavailable(tidal, &ids).await;
    let tracks = candidates
        .into_iter()
        .filter(|t| !unavailable.contains(&t.id))
        .collect();
    Ok(finish(tidal, tracks, NEVER_LIMIT).await)
}

/// Filtres de contenu et blocages, puis plafond de la liste.
async fn finish(tidal: &TidalClient, tracks: Vec<Track>, limit: usize) -> Vec<Track> {
    let mut tracks = tidal.without_blocked(filtered(tracks)).await;
    tracks.truncate(limit);
    tracks
}

/// Titres de l'historique dans l'ordre de la requête, avec leurs métadonnées enregistrées
/// ou complétées ; un titre indisponible ou introuvable est écarté.
fn assemble(
    counted: Vec<CountedTrack>,
    fetched: &[Track],
    unavailable: &HashSet<TrackId>,
) -> Vec<Track> {
    counted
        .into_iter()
        .filter(|(id, _, _)| !unavailable.contains(id))
        .filter_map(|(id, track, _)| track.or_else(|| fetched.iter().find(|t| t.id == id).cloned()))
        .collect()
}

/// Titres likés jamais écoutés, du plus récemment liké au plus ancien (date inconnue à la
/// fin, dans l'ordre reçu).
fn unplayed(mut liked: Vec<TrackEntry>, played: &HashSet<TrackId>) -> Vec<Track> {
    liked.retain(|e| !played.contains(&e.track.id));
    liked.sort_by_key(|e| std::cmp::Reverse(e.added_at.unwrap_or(i64::MIN)));
    liked.into_iter().map(|e| e.track).collect()
}

/// Titres indisponibles parmi `ids` (vérifiés en parallèle, retenus pour la session). Une
/// vérification qui échoue ne masque rien.
async fn unavailable(tidal: &Arc<TidalClient>, ids: &[TrackId]) -> HashSet<TrackId> {
    let mut out = HashSet::new();
    for chunk in ids.chunks(CHECK_PARALLEL) {
        let mut set = tokio::task::JoinSet::new();
        for &id in chunk {
            let tidal = tidal.clone();
            set.spawn(async move { (id, tidal.is_available(id).await) });
        }
        while let Some(result) = set.join_next().await {
            match result {
                Ok((id, Some(false))) => {
                    out.insert(id);
                }
                Ok(_) => {}
                Err(e) => tracing::debug!("tâche interrompue : {e}"),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::{AlbumRef, ArtistRef};

    fn track(id: u64) -> Track {
        Track {
            id: TrackId(id),
            title: format!("Titre {id}"),
            version: None,
            duration: 200,
            artists: vec![ArtistRef {
                id: 1,
                name: "Artiste".into(),
            }],
            album: AlbumRef {
                id: 1,
                title: "Album".into(),
                cover_url: None,
                cover_small_url: None,
                cover_large_url: None,
                color: None,
            },
            explicit: false,
            ai: false,
            hires: false,
            isrc: None,
        }
    }

    fn ids(tracks: &[Track]) -> Vec<u64> {
        tracks.iter().map(|t| t.id.0).collect()
    }

    #[test]
    fn assemble_keeps_order_and_drops_unknown_or_unavailable() {
        let counted = vec![
            (TrackId(1), Some(track(1)), 5),
            (TrackId(2), None, 4),
            (TrackId(3), Some(track(3)), 3),
            (TrackId(4), None, 2),
        ];
        // 2 complété, 3 indisponible, 4 introuvable.
        let tracks = assemble(counted, &[track(2)], &HashSet::from([TrackId(3)]));
        assert_eq!(ids(&tracks), [1, 2]);
    }

    #[test]
    fn unplayed_likes_newest_first() {
        let entry = |id, added_at| TrackEntry {
            track: track(id),
            added_at,
        };
        let liked = vec![
            entry(1, Some(10)),
            entry(2, None),
            entry(3, Some(30)),
            entry(4, Some(20)),
        ];
        let played = HashSet::from([TrackId(4)]);
        assert_eq!(ids(&unplayed(liked, &played)), [3, 1, 2]);
    }
}
