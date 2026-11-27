//! Local smart playlists (#383): fixed lists computed from the listening history
//! of this device (`store::smart`) and the liked tracks, without writing anything to
//! TIDAL. The result goes through the same filters as what the service offers: unavailable
//! tracks, content filter (explicit, AI) and blocked tracks or artists.

use std::collections::HashSet;
use std::sync::Arc;

use tauri::State;

use super::play_history::now_ms;
use super::play_stats::{blocking, fetch};
use super::{AppState, CmdResult, err};
use crate::content::filtered;
use crate::store::{CountedTrack, SmartList, Span, Store};
use crate::tidal::TidalClient;
use crate::tidal::models::{Track, TrackEntry, TrackId};

const DAY_MS: i64 = 24 * 3600 * 1000;
/// "My top 50 of the month".
const TOP_DAYS: i64 = 30;
const TOP_LIMIT: usize = 50;
/// "On repeat": at least `REPEAT_MIN` plays over `REPEAT_DAYS` days.
const REPEAT_DAYS: i64 = 7;
const REPEAT_MIN: i64 = 3;
/// "Forgotten": at least `FORGOTTEN_MIN` plays, none for `FORGOTTEN_DAYS` days.
const FORGOTTEN_DAYS: i64 = 180;
const FORGOTTEN_MIN: i64 = 5;
const LIMIT: usize = 50;
/// "Liked, never played".
const NEVER_LIMIT: usize = 100;
/// Margin of extra candidates requested, to fill the list despite the discarded tracks.
const SPARE: usize = 2;
/// Tracks checked in parallel.
const CHECK_PARALLEL: usize = 6;

/// Tracks of the list, in its order.
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
            let rows = blocking(store, move |s| {
                s.top_tracks(Span::since(since), 1, TOP_LIMIT * SPARE)
            });
            (rows.await?, TOP_LIMIT)
        }
        SmartList::OnRepeat => {
            let since = now - REPEAT_DAYS * DAY_MS;
            let rows = blocking(store, move |s| {
                s.top_tracks(Span::since(since), REPEAT_MIN, LIMIT * SPARE)
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
    // Before the metadata: the check puts the tracks in the catalog cache.
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
            tracing::warn!("history tracks not completed: {e}");
        }
    }
    let tracks = assemble(counted, &fetched, &unavailable);
    Ok(finish(tidal, tracks, limit).await)
}

/// Liked tracks absent from the history, from the most recent like to the oldest.
async fn never_played(tidal: &Arc<TidalClient>, store: &Arc<Store>) -> CmdResult<Vec<Track>> {
    let liked = tidal.favorite_tracks().await.map_err(err)?;
    let played = blocking(store, |s| s.played_track_ids()).await?;
    // What is hidden does not count among the candidates to check.
    let candidates: Vec<Track> = filtered(unplayed(liked.items, &played));
    let candidates: Vec<Track> = candidates.into_iter().take(NEVER_LIMIT * SPARE).collect();
    let ids: Vec<TrackId> = candidates.iter().map(|t| t.id).collect();
    // Already known from reading the favorites: no request in practice.
    let unavailable = unavailable(tidal, &ids).await;
    let tracks = candidates
        .into_iter()
        .filter(|t| !unavailable.contains(&t.id))
        .collect();
    Ok(finish(tidal, tracks, NEVER_LIMIT).await)
}

/// Content filters and blocks, then the list's cap.
async fn finish(tidal: &TidalClient, tracks: Vec<Track>, limit: usize) -> Vec<Track> {
    let mut tracks = tidal.without_blocked(filtered(tracks)).await;
    tracks.truncate(limit);
    tracks
}

/// Tracks of the history in the order of the query, with their recorded or completed
/// metadata; an unavailable or missing track is discarded.
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

/// Liked tracks never played, from the most recently liked to the oldest (unknown date at
/// the end, in the order received).
fn unplayed(mut liked: Vec<TrackEntry>, played: &HashSet<TrackId>) -> Vec<Track> {
    liked.retain(|e| !played.contains(&e.track.id));
    liked.sort_by_key(|e| std::cmp::Reverse(e.added_at.unwrap_or(i64::MIN)));
    liked.into_iter().map(|e| e.track).collect()
}

/// Unavailable tracks among `ids` (checked in parallel, remembered for the session). A
/// failed check hides nothing.
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
                Err(e) => tracing::debug!("task interrupted: {e}"),
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
        // 2 completed, 3 unavailable, 4 not found.
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
