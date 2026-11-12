//! Statistiques d'écoute locales (#326) : titres, albums et artistes les plus écoutés sur
//! une période, temps d'écoute. Calculées en SQL sur l'historique de cet appareil
//! (`store::stats`) ; seuls les titres d'anciennes écoutes (avant la 0.5.33, enregistrées
//! sans leurs métadonnées) et les photos des artistes sont demandés à TIDAL, une fois, puis
//! enregistrés. L'API v1 n'a pas de lecture groupée des titres (`tracks?ids=` répond 400
//! « ISRC is missing », vérifié le 2026-11-12) : ceux du classement sont complétés tout de
//! suite, les autres en arrière-plan, une fois par session, pour le temps d'écoute.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tauri::State;

use super::play_history::now_ms;
use super::{AppState, CmdResult, err};
use crate::store::Store;
use crate::tidal::TidalClient;
use crate::tidal::models::{AlbumSummary, ArtistSummary, Track, TrackId};

const TRACK_LIMIT: usize = 50;
const CARD_LIMIT: usize = 24;
/// Titres complétés en parallèle.
const FETCH_PARALLEL: usize = 6;
/// Anciens titres complétés en arrière-plan au plus, par session.
const BACKFILL_LIMIT: usize = 2_000;
static BACKFILL_STARTED: AtomicBool = AtomicBool::new(false);
const DAY_MS: i64 = 24 * 3600 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StatsPeriod {
    Week,
    Month,
    Year,
    All,
}

impl StatsPeriod {
    /// Début de la période (ms) : fenêtres glissantes, 0 pour tout l'historique.
    fn since(self, now: i64) -> i64 {
        match self {
            Self::Week => now - 7 * DAY_MS,
            Self::Month => now - 30 * DAY_MS,
            Self::Year => now - 365 * DAY_MS,
            Self::All => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counted<T> {
    pub item: T,
    pub plays: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayStats {
    pub plays: i64,
    /// Temps d'écoute approché : somme des durées des titres écoutés, en secondes.
    pub seconds: i64,
    /// Écoutes dont la durée est inconnue, absentes de `seconds`.
    pub unknown: i64,
    pub tracks: Vec<Counted<Track>>,
    pub albums: Vec<Counted<AlbumSummary>>,
    pub artists: Vec<Counted<ArtistSummary>>,
}

#[tauri::command]
pub async fn play_stats(state: State<'_, AppState>, period: StatsPeriod) -> CmdResult<PlayStats> {
    let Some(store) = state.store.clone() else {
        return Ok(PlayStats::default());
    };
    let since = period.since(now_ms());
    let top = blocking(&store, move |s| s.top_tracks(since, 1, TRACK_LIMIT)).await?;
    let missing: Vec<TrackId> = top
        .iter()
        .filter(|(_, track, _)| track.is_none())
        .map(|(id, _, _)| *id)
        .collect();
    let fetched = fetch(&state.tidal, &missing).await;
    if !fetched.is_empty() {
        let save = fetched.clone();
        if let Err(e) = blocking(&store, move |s| s.save_played_tracks(&save)).await {
            tracing::warn!("titres de l'historique non complétés : {e}");
        }
    }
    let tracks = top
        .into_iter()
        .filter_map(|(id, track, plays)| {
            // Titre introuvable (retiré du catalogue) : absent du classement.
            let item = track.or_else(|| fetched.iter().find(|t| t.id == id).cloned())?;
            Some(Counted { item, plays })
        })
        .collect();
    let mut stats = blocking(&store, move |s| {
        let totals = s.play_totals(since)?;
        let albums = s.top_albums(since, 1, CARD_LIMIT)?;
        Ok(PlayStats {
            plays: totals.plays,
            seconds: totals.seconds,
            unknown: totals.unknown,
            tracks,
            albums: albums
                .into_iter()
                .map(|(item, plays)| Counted { item, plays })
                .collect(),
            artists: Vec::new(),
        })
    })
    .await?;
    stats.artists = artists(&state.tidal, &store, since).await?;
    if stats.unknown > 0 && !BACKFILL_STARTED.swap(true, Ordering::Relaxed) {
        tauri::async_runtime::spawn(backfill(state.tidal.clone(), store));
    }
    Ok(stats)
}

/// Complète les titres des anciennes écoutes hors du classement (durée, pour le temps
/// d'écoute). Un titre introuvable reste inconnu.
async fn backfill(tidal: Arc<TidalClient>, store: Arc<Store>) {
    let ids = match blocking(&store, |s| s.unknown_tracks(BACKFILL_LIMIT)).await {
        Ok(ids) => ids,
        Err(e) => return tracing::warn!("historique illisible : {e}"),
    };
    let tracks = fetch(&tidal, &ids).await;
    tracing::info!(
        "historique d'écoute : {}/{} titres complétés",
        tracks.len(),
        ids.len()
    );
    if let Err(e) = blocking(&store, move |s| s.save_played_tracks(&tracks)).await {
        tracing::warn!("titres de l'historique non complétés : {e}");
    }
}

/// Artistes du classement, avec leur photo : celles qui n'ont jamais été demandées le sont
/// à TIDAL puis enregistrées (un échec laisse l'artiste sans photo, retenté plus tard).
async fn artists(
    tidal: &Arc<TidalClient>,
    store: &Arc<Store>,
    since: i64,
) -> CmdResult<Vec<Counted<ArtistSummary>>> {
    let top = blocking(store, move |s| s.top_artists(since, CARD_LIMIT)).await?;
    let missing: Vec<u64> = top
        .iter()
        .filter(|a| !a.picture_known)
        .map(|a| a.artist.id)
        .collect();
    let mut found = HashMap::new();
    if !missing.is_empty() {
        match tidal.artists_by_id(&missing).await {
            Ok(artists) => {
                found = artists;
                // Artiste disparu : enregistré sans photo, comme un artiste qui n'en a pas.
                let pictures: Vec<(u64, Option<String>)> = missing
                    .iter()
                    .map(|id| (*id, found.get(id).and_then(|a| a.picture_url.clone())))
                    .collect();
                if let Err(e) = blocking(store, move |s| s.save_artist_pictures(&pictures)).await {
                    tracing::warn!("photos des artistes non enregistrées : {e}");
                }
            }
            Err(e) => tracing::debug!("photos des artistes indisponibles : {e}"),
        }
    }
    Ok(top
        .into_iter()
        .map(|mut a| {
            if let Some(found) = found.get(&a.artist.id) {
                a.artist.picture_url = found.picture_url.clone();
            }
            Counted {
                item: a.artist,
                plays: a.plays,
            }
        })
        .collect())
}

pub(super) async fn blocking<T: Send + 'static>(
    store: &Arc<Store>,
    f: impl FnOnce(&Store) -> crate::store::Result<T> + Send + 'static,
) -> CmdResult<T> {
    let store = store.clone();
    tauri::async_runtime::spawn_blocking(move || f(&store))
        .await
        .map_err(err)?
        .map_err(err)
}

/// Métadonnées des titres manquants (cache du catalogue, sinon TIDAL), par petits lots. Un
/// échec est ignoré : le titre sera retenté à la prochaine ouverture.
pub(super) async fn fetch(tidal: &Arc<TidalClient>, ids: &[TrackId]) -> Vec<Track> {
    let mut out = Vec::new();
    for chunk in ids.chunks(FETCH_PARALLEL) {
        let mut set = tokio::task::JoinSet::new();
        for &id in chunk {
            let tidal = tidal.clone();
            set.spawn(async move { tidal.track(id).await });
        }
        while let Some(result) = set.join_next().await {
            match result {
                Ok(Ok(track)) => out.push(track),
                Ok(Err(e)) => tracing::debug!("titre de l'historique introuvable : {e}"),
                Err(e) => tracing::debug!("tâche interrompue : {e}"),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periods() {
        let now = 1_000 * DAY_MS;
        assert_eq!(StatsPeriod::Week.since(now), 993 * DAY_MS);
        assert_eq!(StatsPeriod::Year.since(now), 635 * DAY_MS);
        assert_eq!(StatsPeriod::All.since(now), 0);
    }
}
