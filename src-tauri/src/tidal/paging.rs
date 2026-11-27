//! Paged lists: the first page gives `totalNumberOfItems`, the next ones then go out in
//! parallel (at most `CONCURRENCY` requests in flight) and are put back in offset order
//! (#90).

use std::future::Future;
use std::pin::Pin;
use std::task::Poll;

use serde::de::DeserializeOwned;

use super::Result;
use super::api::TidalClient;
use super::models::LibraryList;
use super::raw::RawPage;

/// Most page requests in flight for one list.
const CONCURRENCY: usize = 4;

impl TidalClient {
    /// Walks a paged list up to `max` items. It advances by
    /// `totalNumberOfItems`: a page can hold fewer items than asked (unavailable tracks
    /// filtered by the API). Without a total, sequential fallback: it stops at the first
    /// incomplete page.
    pub(super) async fn get_all<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
        page_size: u32,
        max: usize,
    ) -> Result<Vec<T>> {
        Ok(self.get_list(path, params, page_size, max).await?.items)
    }

    /// Like `get_all`, saying whether the list was cut at the cap (#91).
    pub(super) async fn get_list<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
        page_size: u32,
        max: usize,
    ) -> Result<LibraryList<T>> {
        let first: RawPage<T> = self.page(path, params, page_size, 0).await?;
        let total = first.total_number_of_items;
        let mut received = first.items.len();
        let mut out = first.items;
        let mut offset = page_size;
        match total {
            Some(total) => {
                while offset < total && out.len() < max {
                    let offsets = wave(offset, total, page_size, max - out.len());
                    let Some(&last) = offsets.last() else {
                        break;
                    };
                    // A loop rather than `map`: a closure that returns the future breaks the
                    // `Send` inference of the commands (higher-ranked lifetimes). The
                    // futures only start once polled.
                    let mut requests = Vec::with_capacity(offsets.len());
                    for &o in &offsets {
                        requests.push(self.page::<T>(path, params, page_size, o));
                    }
                    let pages = try_join_bounded(requests, CONCURRENCY).await?;
                    out.extend(pages.into_iter().flat_map(|p| p.items));
                    offset = last + page_size;
                }
            }
            None => {
                while received >= page_size as usize && out.len() < max {
                    let page: RawPage<T> = self.page(path, params, page_size, offset).await?;
                    received = page.items.len();
                    out.extend(page.items);
                    offset += page_size;
                }
            }
        }
        let truncated = is_truncated(out.len(), max, total, offset, received, page_size);
        out.truncate(max);
        Ok(LibraryList {
            items: out,
            truncated,
            total,
        })
    }

    async fn page<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
        page_size: u32,
        offset: u32,
    ) -> Result<RawPage<T>> {
        let mut query = params.to_vec();
        query.push(("limit", page_size.to_string()));
        query.push(("offset", offset.to_string()));
        self.get(path, &query).await
    }
}

/// Offsets of the next wave: enough to fill `missing` items, without exceeding `total`.
/// If unavailable items leave the list under the cap, `get_all` starts another wave for
/// the rest.
fn wave(offset: u32, total: u32, page_size: u32, missing: usize) -> Vec<u32> {
    let pages = missing.div_ceil(page_size as usize);
    (0..pages)
        .map_while(|i| {
            let o = u32::try_from(i)
                .ok()?
                .checked_mul(page_size)?
                .checked_add(offset)?;
            (o < total).then_some(o)
        })
        .collect()
}

/// The list stopped at the cap while more items followed: more than `max` received, or
/// pages remaining (`next` = offset of the next page). This is not `total > len`:
/// unavailable tracks are counted in the total but filtered out of the pages.
fn is_truncated(
    len: usize,
    max: usize,
    total: Option<u32>,
    next: u32,
    last_received: usize,
    page_size: u32,
) -> bool {
    len > max
        || match total {
            Some(total) => next < total,
            None => len >= max && last_received >= page_size as usize,
        }
}

/// Awaits all the futures, at most `limit` at a time (started in order), and returns
/// their results in input order; the first error abandons the others.
/// Equivalent of `buffered(limit)` + `try_collect` without the `futures` crate.
pub(crate) async fn try_join_bounded<F, T, E>(
    futures: impl IntoIterator<Item = F>,
    limit: usize,
) -> std::result::Result<Vec<T>, E>
where
    F: Future<Output = std::result::Result<T, E>>,
{
    let limit = limit.max(1);
    let mut pending = futures.into_iter().enumerate();
    let mut running: Vec<(usize, Pin<Box<F>>)> = Vec::new();
    let mut results: Vec<Option<T>> = Vec::new();
    std::future::poll_fn(|cx| {
        loop {
            while running.len() < limit {
                let Some((i, f)) = pending.next() else {
                    break;
                };
                results.push(None);
                running.push((i, Box::pin(f)));
            }
            if running.is_empty() {
                // All done: every slot is filled.
                return Poll::Ready(Ok(std::mem::take(&mut results)
                    .into_iter()
                    .flatten()
                    .collect()));
            }
            let mut finished = false;
            let mut k = 0;
            while k < running.len() {
                match running[k].1.as_mut().poll(cx) {
                    Poll::Ready(Ok(value)) => {
                        results[running[k].0] = Some(value);
                        running.swap_remove(k);
                        finished = true;
                    }
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => k += 1,
                }
            }
            // A slot freed up: start the next one right away.
            if !finished {
                return Poll::Pending;
            }
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn wave_covers_what_is_missing_without_passing_total() {
        assert_eq!(
            wave(100, 2_500, 100, 1_900),
            (1..20).map(|i| i * 100).collect::<Vec<_>>()
        );
        // Total reached before the cap.
        assert_eq!(wave(100, 350, 100, 9_900), [100, 200, 300]);
        // A few missing items (unavailable tracks): one more page.
        assert_eq!(wave(2_000, 2_500, 100, 3), [2_000]);
        assert!(wave(400, 350, 100, 10).is_empty());
        assert!(wave(100, 1_000, 100, 0).is_empty());
    }

    #[test]
    fn truncation_is_reported_only_when_items_remain() {
        // Full list, a few unavailable tracks filtered: not cut.
        assert!(!is_truncated(424, 10_000, Some(426), 500, 24, 100));
        // Cap reached with pages remaining.
        assert!(is_truncated(10_000, 10_000, Some(12_345), 10_000, 100, 100));
        // Catch-up wave that exceeds the cap, even on the last page.
        assert!(is_truncated(10_040, 10_000, Some(10_050), 10_100, 50, 100));
        // Exactly the cap, nothing after.
        assert!(!is_truncated(
            10_000,
            10_000,
            Some(10_000),
            10_000,
            100,
            100
        ));
        // Without a total: cut only if the last page was full.
        assert!(is_truncated(2_000, 2_000, None, 2_000, 100, 100));
        assert!(!is_truncated(1_950, 2_000, None, 2_000, 50, 100));
    }

    #[tokio::test]
    async fn bounded_join_keeps_order_and_limit() {
        let in_flight = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let futures = (0..10).map(|i| {
            let (in_flight, peak) = (&in_flight, &peak);
            async move {
                let now = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                // The last ones finish first: the order must stay the input order.
                for _ in 0..(10 - i) {
                    tokio::task::yield_now().await;
                }
                in_flight.fetch_sub(1, Ordering::SeqCst);
                Ok::<_, ()>(i)
            }
        });
        let out = try_join_bounded(futures, 4).await.unwrap();
        assert_eq!(out, (0..10).collect::<Vec<_>>());
        assert_eq!(peak.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn bounded_join_stops_at_first_error() {
        let started = AtomicUsize::new(0);
        let futures = (0..10).map(|i| {
            let started = &started;
            async move {
                started.fetch_add(1, Ordering::SeqCst);
                if i == 1 { Err(i) } else { Ok(i) }
            }
        });
        assert_eq!(try_join_bounded(futures, 2).await, Err(1));
        assert!(started.load(Ordering::SeqCst) < 10);
        let empty: Vec<std::future::Ready<std::result::Result<u8, ()>>> = Vec::new();
        assert_eq!(try_join_bounded(empty, 4).await, Ok(Vec::new()));
    }
}
