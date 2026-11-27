//! Local listening statistics (#326): most played tracks, albums and artists over a
//! period, listening time. Computed in SQL over this device's history
//! (`store::stats`); only the tracks of old plays (before 0.5.33, recorded
//! without their metadata) and the artists' pictures are requested from TIDAL, once, then
//! stored. The v1 API has no grouped read of tracks (`tracks?ids=` answers 400
//! "ISRC is missing", checked on 2026-11-12): the ranking's tracks are completed right
//! away, the others in the background, once per session, for the listening time.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tauri::State;

use super::play_history::now_ms;
use super::{AppState, CmdResult, err};
use crate::store::{CountedTrack, Span, Store, TopArtist};
use crate::tidal::TidalClient;
use crate::tidal::models::{AlbumSummary, ArtistSummary, Track, TrackId};

const TRACK_LIMIT: usize = 50;
const CARD_LIMIT: usize = 24;
/// Tracks completed in parallel.
const FETCH_PARALLEL: usize = 6;
/// Old tracks completed in the background at most, per session.
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
    /// Start of the period (ms): sliding windows, 0 for the whole history.
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
    /// Approximate listening time: sum of the durations of the tracks played, in seconds.
    pub seconds: i64,
    /// Plays whose duration is unknown, missing from `seconds`.
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
    let span = Span::since(period.since(now_ms()));
    let top = blocking(&store, move |s| s.top_tracks(span, 1, TRACK_LIMIT)).await?;
    let tracks = complete(&state.tidal, &store, top).await;
    let mut stats = blocking(&store, move |s| {
        let totals = s.play_totals(span)?;
        let albums = s.top_albums(span, 1, CARD_LIMIT)?;
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
    stats.artists = artists(&state.tidal, &store, span, CARD_LIMIT).await?;
    if stats.unknown > 0 {
        start_backfill(&state.tidal, &store);
    }
    Ok(stats)
}

/// Starts `backfill` once per session.
pub(super) fn start_backfill(tidal: &Arc<TidalClient>, store: &Arc<Store>) {
    if !BACKFILL_STARTED.swap(true, Ordering::Relaxed) {
        tauri::async_runtime::spawn(backfill(tidal.clone(), store.clone()));
    }
}

/// Ranking of tracks: those of old plays recorded without metadata are
/// requested from TIDAL and stored; a track not found (removed from the catalog) drops out.
pub(super) async fn complete(
    tidal: &Arc<TidalClient>,
    store: &Arc<Store>,
    top: Vec<CountedTrack>,
) -> Vec<Counted<Track>> {
    let missing: Vec<TrackId> = top
        .iter()
        .filter(|(_, track, _)| track.is_none())
        .map(|(id, _, _)| *id)
        .collect();
    let fetched = fetch(tidal, &missing).await;
    if !fetched.is_empty() {
        let save = fetched.clone();
        if let Err(e) = blocking(store, move |s| s.save_played_tracks(&save)).await {
            tracing::warn!("history tracks not completed: {e}");
        }
    }
    top.into_iter()
        .filter_map(|(id, track, plays)| {
            let item = track.or_else(|| fetched.iter().find(|t| t.id == id).cloned())?;
            Some(Counted { item, plays })
        })
        .collect()
}

/// Completes the tracks of old plays outside the ranking (duration, for the listening
/// time). A track not found stays unknown.
async fn backfill(tidal: Arc<TidalClient>, store: Arc<Store>) {
    let ids = match blocking(&store, |s| s.unknown_tracks(BACKFILL_LIMIT)).await {
        Ok(ids) => ids,
        Err(e) => return tracing::warn!("history unreadable: {e}"),
    };
    let tracks = fetch(&tidal, &ids).await;
    tracing::info!(
        "listening history: {}/{} tracks completed",
        tracks.len(),
        ids.len()
    );
    if let Err(e) = blocking(&store, move |s| s.save_played_tracks(&tracks)).await {
        tracing::warn!("history tracks not completed: {e}");
    }
}

/// Artists of the ranking, with their picture.
pub(super) async fn artists(
    tidal: &Arc<TidalClient>,
    store: &Arc<Store>,
    span: Span,
    limit: usize,
) -> CmdResult<Vec<Counted<ArtistSummary>>> {
    let top = blocking(store, move |s| s.top_artists(span, limit)).await?;
    Ok(pictured(tidal, store, top).await)
}

/// Artists' pictures: those never requested are requested from TIDAL then
/// stored (a failure leaves the artist without a picture, retried later).
pub(super) async fn pictured(
    tidal: &Arc<TidalClient>,
    store: &Arc<Store>,
    top: Vec<TopArtist>,
) -> Vec<Counted<ArtistSummary>> {
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
                // Artist gone: stored without a picture, like an artist who has none.
                let pictures: Vec<(u64, Option<String>)> = missing
                    .iter()
                    .map(|id| (*id, found.get(id).and_then(|a| a.picture_url.clone())))
                    .collect();
                if let Err(e) = blocking(store, move |s| s.save_artist_pictures(&pictures)).await {
                    tracing::warn!("artist pictures not stored: {e}");
                }
            }
            Err(e) => tracing::debug!("artist pictures unavailable: {e}"),
        }
    }
    top.into_iter()
        .map(|mut a| {
            if let Some(found) = found.get(&a.artist.id) {
                a.artist.picture_url = found.picture_url.clone();
            }
            Counted {
                item: a.artist,
                plays: a.plays,
            }
        })
        .collect()
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

/// Metadata of the missing tracks (catalog cache, otherwise TIDAL), in small batches. A
/// failure is ignored: the track will be retried at the next opening.
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
                Ok(Err(e)) => tracing::debug!("history track not found: {e}"),
                Err(e) => tracing::debug!("task interrupted: {e}"),
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
