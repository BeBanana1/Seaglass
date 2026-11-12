//! Couverture du fil d'actualité (#415), sur l'API réelle :
//! `cargo test live_feed_coverage -- --ignored --nocapture`.
//!
//! Compare les sorties `NEW_ALBUM_RELEASE` du fil aux albums et singles récents de chaque
//! artiste suivi (`artists/{id}/albums`), pour savoir si le fil suffit, puis lit le fil
//! complété par `releases.rs`.

use std::collections::HashSet;

use super::live_tests::client;
use super::models::MediaItem;

/// Fenêtre comparée, en jours.
const DAYS: i64 = 60;

#[tokio::test]
#[ignore = "réseau et session TIDAL requis"]
async fn live_feed_coverage() {
    let client = client().await;
    let feed = client.feed().await.unwrap();
    let feed_albums: HashSet<u64> = feed
        .iter()
        .filter_map(|f| match &f.item {
            MediaItem::Album(a) => Some(a.id),
            _ => None,
        })
        .collect();
    let oldest = feed.iter().filter_map(|f| f.occurred_at).min();
    println!(
        "fil : {} activités, {} albums, plus ancienne {:?}",
        feed.len(),
        feed_albums.len(),
        oldest.map(chrono_day)
    );

    let artists = client.favorite_artists().await.unwrap();
    let today = chrono_day(now_ms());
    let since = chrono_day(now_ms() - DAYS * 86_400_000);
    println!(
        "{} artistes suivis, sorties depuis {since}",
        artists.items.len()
    );
    let start = std::time::Instant::now();
    let (mut recent, mut missing) = (0, 0);
    for artist in &artists.items {
        let page = match client.artist(artist.id).await {
            Ok(page) => page,
            Err(e) => {
                println!("  {} : erreur {e}", artist.name);
                continue;
            }
        };
        for album in page.albums.iter().chain(&page.singles) {
            let Some(date) = album.release_date.as_deref() else {
                continue;
            };
            if date < since.as_str() || date > today.as_str() {
                continue;
            }
            recent += 1;
            let same_title = feed.iter().any(|f| matches!(&f.item, MediaItem::Album(a) if a.id != album.id && a.title == album.title));
            if same_title {
                println!("    (même titre, autre id, dans le fil)");
            }
            let seen = feed_albums.contains(&album.id);
            if !seen {
                missing += 1;
            }
            println!(
                "  {} {} — {} ({:?}){}",
                if seen { "ok " } else { "ABS" },
                date,
                album.title,
                album.album_type,
                if album.artists.first().map(|a| a.id) == Some(artist.id) {
                    ""
                } else {
                    " [invité]"
                }
            );
        }
    }
    println!(
        "{recent} sorties récentes, {missing} absentes du fil, {} artistes lus en {:?}",
        artists.items.len(),
        start.elapsed()
    );

    // Fil complété (eleases.rs) : sorties ajoutées, éditions en double écartées.
    let client = std::sync::Arc::new(client);
    let start = std::time::Instant::now();
    let merged = client.feed_with_releases(now_ms()).await.unwrap();
    println!(
        "fil complété : {} éléments ({} ajoutés) en {:?}",
        merged.len(),
        merged.len() - feed.len(),
        start.elapsed()
    );
    assert!(merged.len() >= feed.len());
    assert!(
        merged
            .windows(2)
            .all(|w| w[0].occurred_at >= w[1].occurred_at)
    );
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// `AAAA-MM-JJ` d'un instant en ms UTC (algorithme des jours civils de H. Hinnant).
fn chrono_day(ms: i64) -> String {
    let z = ms.div_euclid(86_400_000) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}
