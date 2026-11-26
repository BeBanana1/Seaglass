//! TIDAL matches for the tracks of an imported playlist (#328). Measured on 2026-11-12:
//! `tracks?isrc=` ~0.09 s per track with 4 in flight, `search` ~0.75 s in series, no 429 over ~670
//! requests in 5 min. A 429 still gives a pause and a new attempt.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use super::matching::{rank_editions, rank_tracks, verdict};
use super::playlists::Entry;
use crate::i18n::tr;
use crate::tidal::models::Track;
use crate::tidal::{TidalClient, TidalError, try_join_bounded};

/// TIDAL requests in flight at most (same bound as the paging).
const IN_FLIGHT: usize = 4;
const SEARCH_LIMIT: u32 = 30;
/// Pause before retrying after a 429.
const BACKOFF: Duration = Duration::from_secs(2);
const RETRIES: u32 = 2;

/// A track of the original playlist and what matches it on TIDAL.
#[derive(Debug, Clone)]
pub struct Matched {
    pub entry: Entry,
    /// `None`: nothing similar.
    pub track: Option<Track>,
    /// Title, artist and duration agree (or same ISRC).
    pub sure: bool,
}

/// Looks for each track, `IN_FLIGHT` at a time, in the playlist's order. `progress` receives the
/// number of tracks processed; `cancelled` stops everything (new import).
pub async fn match_all(
    tidal: &TidalClient,
    entries: Vec<Entry>,
    progress: &(dyn Fn(usize) + Sync),
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<Matched>, String> {
    let done = AtomicUsize::new(0);
    // Futures built in a loop (see `paging.rs`: inference of `Send`).
    let mut futures = Vec::with_capacity(entries.len());
    for entry in entries {
        let done = &done;
        futures.push(async move {
            if cancelled() {
                return Err(tr("import annulé", "import cancelled").to_owned());
            }
            let matched = match_one(tidal, entry).await?;
            progress(done.fetch_add(1, Ordering::Relaxed) + 1);
            Ok(matched)
        });
    }
    try_join_bounded(futures, IN_FLIGHT).await
}

async fn match_one(tidal: &TidalClient, entry: Entry) -> Result<Matched, String> {
    let wanted = &entry.wanted;
    if let Some(isrc) = entry
        .isrc
        .as_deref()
        .filter(|i| i.len() == 12 && i.chars().all(|c| c.is_ascii_alphanumeric()))
    {
        let found = retry(|| tidal.tracks_by_isrc(isrc)).await?;
        if let Some(track) = rank_editions(found, wanted.album.as_deref())
            .into_iter()
            .next()
        {
            return Ok(Matched {
                entry,
                track: Some(track),
                sure: true,
            });
        }
    }
    if wanted.title.trim().is_empty() {
        return Ok(Matched {
            entry,
            track: None,
            sure: false,
        });
    }
    let query = wanted.query();
    let results = retry(|| tidal.search(&query, SEARCH_LIMIT)).await?;
    let best = rank_tracks(results.tracks, wanted)
        .into_iter()
        .next()
        .and_then(|t| verdict(&t, wanted).map(|sure| (t, sure)));
    Ok(match best {
        Some((track, sure)) => Matched {
            entry,
            track: Some(track),
            sure,
        },
        None => Matched {
            entry,
            track: None,
            sure: false,
        },
    })
}

/// Retries after a pause on a 429 (rate limit), `RETRIES` times at most.
async fn retry<T, F, Fut>(call: F) -> Result<T, String>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, TidalError>>,
{
    let mut attempt = 0;
    loop {
        match call().await {
            Err(TidalError::Api { status: 429, .. }) if attempt < RETRIES => {
                attempt += 1;
                tokio::time::sleep(BACKOFF * attempt).await;
            }
            other => return other.map_err(|e| e.to_string()),
        }
    }
}
