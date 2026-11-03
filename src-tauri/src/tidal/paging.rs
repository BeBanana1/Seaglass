//! Listes paginées : la première page donne `totalNumberOfItems`, les suivantes partent
//! alors en parallèle (au plus `CONCURRENCY` requêtes en vol) et sont remises dans l'ordre
//! des offsets (#90).

use std::future::Future;
use std::pin::Pin;
use std::task::Poll;

use serde::de::DeserializeOwned;

use super::Result;
use super::api::TidalClient;
use super::models::LibraryList;
use super::raw::RawPage;

/// Requêtes de pages en vol au plus, pour une même liste.
const CONCURRENCY: usize = 4;

impl TidalClient {
    /// Parcourt une liste paginée jusqu'à `max` éléments. On avance selon
    /// `totalNumberOfItems` : une page peut contenir moins d'éléments que demandé (titres
    /// indisponibles filtrés par l'API). Sans total, repli séquentiel : on s'arrête à la
    /// première page incomplète.
    pub(super) async fn get_all<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
        page_size: u32,
        max: usize,
    ) -> Result<Vec<T>> {
        Ok(self.get_list(path, params, page_size, max).await?.items)
    }

    /// Comme `get_all`, en disant si la liste a été coupée au plafond (#91).
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
                    // Boucle plutôt que `map` : une fermeture qui renvoie la future fait
                    // échouer l'inférence de `Send` des commandes (lifetimes d'ordre
                    // supérieur). Les futures ne partent qu'une fois interrogées.
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

/// Offsets de la prochaine vague : de quoi combler `missing` éléments, sans dépasser `total`.
/// Si des éléments indisponibles laissent la liste sous le plafond, `get_all` relance une
/// vague pour le reste.
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

/// La liste s'est arrêtée au plafond alors que d'autres éléments suivaient : plus de `max`
/// reçus, ou des pages restantes (`next` = offset de la page suivante). Ce n'est pas
/// `total > len` : les titres indisponibles sont comptés dans le total mais filtrés des pages.
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

/// Attend toutes les futures, au plus `limit` à la fois (démarrées dans l'ordre), et renvoie
/// leurs résultats dans l'ordre d'entrée ; la première erreur abandonne les autres.
/// Équivalent de `buffered(limit)` + `try_collect` sans la crate `futures`.
pub(super) async fn try_join_bounded<F, T, E>(
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
                // Toutes terminées : chaque case est remplie.
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
            // Une place s'est libérée : démarrer la suivante tout de suite.
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
        // Total atteint avant le plafond.
        assert_eq!(wave(100, 350, 100, 9_900), [100, 200, 300]);
        // Quelques éléments manquants (titres indisponibles) : une page de plus.
        assert_eq!(wave(2_000, 2_500, 100, 3), [2_000]);
        assert!(wave(400, 350, 100, 10).is_empty());
        assert!(wave(100, 1_000, 100, 0).is_empty());
    }

    #[test]
    fn truncation_is_reported_only_when_items_remain() {
        // Liste complète, quelques titres indisponibles filtrés : pas coupée.
        assert!(!is_truncated(424, 10_000, Some(426), 500, 24, 100));
        // Plafond atteint avec des pages restantes.
        assert!(is_truncated(10_000, 10_000, Some(12_345), 10_000, 100, 100));
        // Vague de rattrapage qui dépasse le plafond, même sur la dernière page.
        assert!(is_truncated(10_040, 10_000, Some(10_050), 10_100, 50, 100));
        // Exactement le plafond, rien après.
        assert!(!is_truncated(
            10_000,
            10_000,
            Some(10_000),
            10_000,
            100,
            100
        ));
        // Sans total : coupée seulement si la dernière page était pleine.
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
                // Les dernières finissent en premier : l'ordre doit rester celui d'entrée.
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
