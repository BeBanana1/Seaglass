//! Tests on the real API, ignored by default (network and a saved session required).
//!
//! - `cargo test live_catalog -- --ignored --nocapture`: calls each endpoint.
//! - `cargo test live_lyrics -- --ignored --nocapture`: synced and missing lyrics.
//! - `cargo test live_track_info -- --ignored --nocapture`: detailed track info (#3).
//! - `cargo test live_track_credits -- --ignored --nocapture`: credits of a track (#84).
//! - `cargo test live_album_credits -- --ignored --nocapture`: credits of an album (#206).
//! - `cargo test live_contributor -- --ignored --nocapture`: a contributor's page (#429).
//! - `cargo test live_album_related -- --ignored --nocapture`: other versions of an album
//!   (#350).
//! - `cargo test live_hires_twin_album -- --ignored --nocapture`: hi-res twin album (#59).
//! - `cargo test live_locale -- --ignored --nocapture`: home titles in English (#7).
//! - `cargo test live_home_pages -- --ignored --nocapture`: all the home pages (#207).
//! - `cargo test live_blocks -- --ignored --nocapture`: blocks, in `live_blocks.rs`
//!   (**modifies the account**, then restores it).
//! - `cargo test live_cleanup -- --ignored --nocapture`: list cleanup (#382), in
//!   `live_cleanup.rs` (`live_cleanup_apply` **creates then deletes a playlist**).
//! - `cargo test live_collection -- --ignored --nocapture`: library writes, in
//!   `live_collection.rs` (**modifies the account**).
//! - `cargo test live_feed_coverage -- --ignored --nocapture`: coverage of the activity
//!   feed and added releases (#415), in `live_feed.rs`.
//! - `cargo test live_folders -- --ignored --nocapture`: playlist folders (#331), in
//!   `live_folders.rs` (**creates then deletes a folder and two playlists**).
//! - `cargo test dump_ui_fixtures -- --ignored`: writes `src/lib/mock/fixtures.json`
//!   (gitignored, contains the account's library) to preview the UI outside Tauri.

use super::TidalClient;
use super::models::{LyricsText, MediaItem, RadioSeed, TrackId};

pub(super) const ALBUM_ID: u64 = 156_749_593;
const ARTIST_ID: u64 = 12_979;
/// "Bohemian Rhapsody": timed lyrics (Musixmatch).
const LYRICS_TRACK: TrackId = TrackId(534_050_211);
/// "Clair de lune": instrumental, `tracks/{id}/lyrics` answers 404.
const NO_LYRICS_TRACK: TrackId = TrackId(5_940_831);

pub(super) async fn client() -> TidalClient {
    let client = TidalClient::new().unwrap();
    assert!(
        client.current_session().await.unwrap().is_some(),
        "no session"
    );
    client
}

/// Home follows `page.cursor`: more rows than the first page (11 on 2026-11-05), unique
/// keys.
#[tokio::test]
#[ignore = "network and TIDAL session required"]
async fn live_home_pages() {
    let client = client().await;
    // First call: token refresh included; the second measures home alone.
    client.home().await.unwrap();
    let start = std::time::Instant::now();
    let home = client.home().await.unwrap();
    println!("{} rows in {:?}", home.len(), start.elapsed());
    for row in &home {
        println!("  {}: {} ({} items)", row.key, row.title, row.items.len());
    }
    let mut keys: Vec<&str> = home.iter().map(|r| r.key.as_str()).collect();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), home.len(), "duplicate keys");
    assert!(home.len() > 11, "only the first page?");
}

/// The `locale` follows the app's language: same home rows (same `key`), translated
/// titles. Changes the global language for the test's duration: run it alone.
#[tokio::test]
#[ignore = "network and TIDAL session required, changes the global language"]
async fn live_locale() {
    use crate::i18n::{self, Lang};
    let client = client().await;
    i18n::set(Lang::En);
    let en = client.home().await;
    i18n::set(Lang::Fr);
    let en = en.unwrap();
    let fr = client.home().await.unwrap();
    for row in &en {
        let fr_title = fr
            .iter()
            .find(|r| r.key == row.key)
            .map(|r| r.title.as_str());
        println!("{}: {:?} / {:?}", row.key, row.title, fr_title);
    }
    let mix = en.iter().flat_map(|r| &r.items).find_map(|i| match i {
        MediaItem::Mix(m) => Some(m),
        _ => None,
    });
    println!("mix: {:?}", mix.map(|m| (&m.title, &m.subtitle)));
    let translated = en
        .iter()
        .filter(|r| fr.iter().any(|f| f.key == r.key && f.title != r.title))
        .count();
    assert!(translated > 0, "no row title changes with the locale");
}

#[tokio::test]
#[ignore = "network and TIDAL session required"]
async fn live_catalog() {
    let client = client().await;
    let search = client.search("interstellar", 5).await.unwrap();
    println!(
        "search: {} tracks, {} albums, {} artists, {} playlists",
        search.tracks.len(),
        search.albums.len(),
        search.artists.len(),
        search.playlists.len()
    );
    let album = client.album(ALBUM_ID).await.unwrap();
    println!(
        "album: {} ({} tracks, color {:?})",
        album.album.title,
        album.tracks.len(),
        album.album.color
    );
    assert_eq!(album.tracks.len(), 30);
    assert!(album.tracks[0].album.color.is_some());
    let artist = client.artist(ARTIST_ID).await.unwrap();
    println!(
        "artist: {} ({} top, {} albums, {} singles)",
        artist.artist.name,
        artist.top_tracks.len(),
        artist.albums.len(),
        artist.singles.len()
    );
    let playlist = client.playlist(&search.playlists[0].uuid).await.unwrap();
    println!(
        "playlist: {} ({} tracks)",
        playlist.playlist.title,
        playlist.tracks.len()
    );
    // `dateAdded` is provided for each playlist track (verified on 2026-10-30).
    assert!(playlist.tracks.iter().all(|t| t.added_at.is_some()));
    // Paging (#90): loading time of a big playlist and of the favorites.
    let mut found = client.library_playlists().await.unwrap().items;
    for query in [
        "1000 songs",
        "top 1000",
        "every song",
        "complete discography",
    ] {
        found.extend(client.search(query, 50).await.unwrap().playlists);
    }
    let big = found
        .into_iter()
        .max_by_key(|p| p.number_of_tracks.unwrap_or(0))
        .unwrap();
    let started = std::time::Instant::now();
    let page = client.playlist(&big.uuid).await.unwrap();
    println!(
        "big playlist: {} ({} tracks out of {:?}) in {:?}",
        page.playlist.title,
        page.tracks.len(),
        big.number_of_tracks,
        started.elapsed()
    );
    let started = std::time::Instant::now();
    let liked = client.favorite_tracks().await.unwrap();
    // 424 tracks on the test account: complete list (the total also counts unavailable ones).
    assert!(!liked.truncated);
    println!("announced total: {:?}", liked.total);
    let favorites = liked.items;
    println!(
        "liked tracks: {} in {:?}",
        favorites.len(),
        started.elapsed()
    );
    assert!(favorites.iter().all(|t| t.added_at.is_some()));
    // Pages requested in parallel, put back in order: always most recent to oldest.
    assert!(favorites.windows(2).all(|w| w[0].added_at >= w[1].added_at));
    println!(
        "library: {} tracks, {} albums, {} artists, {} playlists",
        favorites.len(),
        client.favorite_albums().await.unwrap().items.len(),
        client.favorite_artists().await.unwrap().items.len(),
        client.library_playlists().await.unwrap().items.len()
    );
    let home = client.home().await.unwrap();
    for row in &home {
        println!("home: {} ({} items)", row.title, row.items.len());
    }
    assert!(!home.is_empty());
    let feed = client.feed().await.unwrap();
    println!(
        "feed: {} releases, {} unseen",
        feed.len(),
        feed.iter().filter(|f| !f.seen).count()
    );
    assert!(feed.iter().all(|f| f.occurred_at.is_some()));
    let mixes: Vec<_> = feed
        .iter()
        .filter_map(|f| match &f.item {
            super::models::MediaItem::Mix(m) => Some(m),
            _ => None,
        })
        .collect();
    println!("feed: {} mixes", mixes.len());
    // The feed's monthly mix opens like the others (`pages/mix` + `mixes/{id}/items`).
    if let Some(mix) = mixes.first() {
        let page = client.mix(&mix.id).await.unwrap();
        println!(
            "feed mix: {} ({} tracks)",
            page.mix.title,
            page.tracks.len()
        );
        assert!(!page.tracks.is_empty());
    }
    let mixes = client.favorite_mixes().await.unwrap();
    println!("saved mixes: {}", mixes.len());
    let home_mix = home.iter().flat_map(|r| &r.items).find_map(|i| match i {
        MediaItem::Mix(m) => Some(m.id.clone()),
        _ => None,
    });
    if let Some(id) = home_mix.or_else(|| mixes.first().map(|m| m.id.clone())) {
        let mix = client.mix(&id).await.unwrap();
        println!("mix: {} ({} tracks)", mix.mix.title, mix.tracks.len());
        assert!(!mix.tracks.is_empty());
        assert!(mix.image_large_url.is_some());
    }
    let track_radio = client
        .radio(RadioSeed::Track, album.tracks[0].id.0)
        .await
        .unwrap();
    let artist_radio = client.radio(RadioSeed::Artist, ARTIST_ID).await.unwrap();
    println!(
        "radios: {} ({} tracks), {} ({} tracks)",
        track_radio.title,
        track_radio.tracks.len(),
        artist_radio.title,
        artist_radio.tracks.len()
    );
    assert!(!track_radio.tracks.is_empty() && !artist_radio.tracks.is_empty());
}

#[tokio::test]
#[ignore = "network and TIDAL session required"]
async fn dump_ui_fixtures() {
    let client = client().await;
    let search = client.search("interstellar", 20).await.unwrap();
    let playlist = client.playlist(&search.playlists[0].uuid).await.unwrap();
    let mut tracks = client.favorite_tracks().await.unwrap().items;
    tracks.truncate(100);
    let album = client.album(ALBUM_ID).await.unwrap();
    let mixes = client.favorite_mixes().await.unwrap();
    let mix = match mixes.first() {
        Some(m) => Some(client.mix(&m.id).await.unwrap()),
        None => None,
    };
    let radio = client
        .radio(RadioSeed::Track, album.tracks[0].id.0)
        .await
        .unwrap();
    let fixtures = serde_json::json!({
        "search": search,
        "album": album,
        "mix": mix,
        "radio": radio,
        "artist": client.artist(ARTIST_ID).await.unwrap(),
        "playlist": playlist,
        "home": client.home().await.unwrap(),
        "feed": client.feed().await.unwrap(),
        "library": {
            "tracks": tracks,
            "albums": client.favorite_albums().await.unwrap().items,
            "artists": client.favorite_artists().await.unwrap().items,
            "playlists": client.library_playlists().await.unwrap().items,
            "mixes": mixes,
        },
    });
    let path = std::path::Path::new("../src/lib/mock/fixtures.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_string(&fixtures).unwrap()).unwrap();
}

/// `cargo test live_hires -- --ignored --nocapture`: *Billie Jean* from "Thriller 25 Super
/// Deluxe Edition" only exists in 16/44.1, the same ISRC is hi-res on other editions.
#[tokio::test]
#[ignore = "network and TIDAL session required"]
async fn live_hires() {
    use super::models::{AudioQuality, StreamUse, TrackId};

    let client = client().await;
    let track = client.track(TrackId(1_632_061)).await.unwrap();
    assert!(!track.hires);
    assert_eq!(track.isrc.as_deref(), Some("USSM19902991"));
    let start = std::time::Instant::now();
    let edition = client.hires_edition(&track).await.unwrap().unwrap();
    println!(
        "hi-res edition: {} ({}) in {:?}",
        edition.id,
        edition.album.title,
        start.elapsed()
    );
    assert!(edition.hires);
    assert!(client.known_hires_edition(track.id).is_some());
    let stream = client
        .stream(edition.id, AudioQuality::HiResLossless, StreamUse::Play)
        .await
        .unwrap();
    println!("stream: {:?}", stream.format);
    assert_eq!(stream.format.bit_depth, Some(24));
    // A track that is already hi-res is never replaced.
    assert!(client.hires_edition(&edition).await.unwrap().is_none());
}

/// `cargo test live_hires_twin -- --ignored --nocapture`: *Hotel California (2013
/// Remaster)* (`20767339`, 16/44.1) has a hi-res twin album (`68709104`, 24/192) under
/// other ISRCs (#55). The other tracks of the album come from the cache.
#[tokio::test]
#[ignore = "network and TIDAL session required"]
async fn live_hires_twin() {
    use super::models::{AudioQuality, StreamUse};

    let client = client().await;
    let album = client.album(20_767_338).await.unwrap();
    let (first, second) = (&album.tracks[0], &album.tracks[1]);
    assert_eq!(first.id, TrackId(20_767_339));
    assert!(!first.hires);
    let start = std::time::Instant::now();
    let edition = client.hires_edition(first).await.unwrap().unwrap();
    println!(
        "twin: {} ({}, {}) in {:?}",
        edition.id,
        edition.album.title,
        edition.album.id,
        start.elapsed()
    );
    assert_eq!(edition.album.id, 68_709_104);
    assert_ne!(edition.isrc, first.isrc);
    let start = std::time::Instant::now();
    let next = client.hires_edition(second).await.unwrap().unwrap();
    println!("{} → {} in {:?}", second.title, next.id, start.elapsed());
    assert_eq!(next.album.id, 68_709_104);
    let stream = client
        .stream(edition.id, AudioQuality::HiResLossless, StreamUse::Play)
        .await
        .unwrap();
    println!("stream: {:?}", stream.format);
    assert_eq!(stream.format.bit_depth, Some(24));
    assert_eq!(stream.format.sample_rate, Some(192_000));
}

/// `cargo test live_hires_twin_album -- --ignored --nocapture`: the page of "Hotel
/// California (2013 Remaster)" in 16/44.1 offers its hi-res twin (#59), not the reverse.
#[tokio::test]
#[ignore = "network and TIDAL session required"]
async fn live_hires_twin_album() {
    let client = client().await;
    let start = std::time::Instant::now();
    let twin = client.hires_twin_album(20_767_338).await.unwrap().unwrap();
    println!(
        "twin: {} ({}) in {:?}",
        twin.id,
        twin.title,
        start.elapsed()
    );
    assert_eq!(twin.id, 68_709_104);
    assert!(twin.hires);
    assert!(client.hires_twin_album(68_709_104).await.unwrap().is_none());
}

#[tokio::test]
#[ignore = "network and TIDAL session required"]
async fn live_lyrics() {
    let client = client().await;
    let lyrics = client.lyrics(LYRICS_TRACK).await.unwrap().unwrap();
    let LyricsText::Synced { lines } = &lyrics.text else {
        panic!("lyrics not synchronized");
    };
    println!(
        "lyrics: {} lines ({:?}), first at {} ms: {}",
        lines.len(),
        lyrics.provider,
        lines[0].time_ms,
        lines[0].text
    );
    assert!(lines.windows(2).all(|w| w[0].time_ms <= w[1].time_ms));
    assert!(client.lyrics(NO_LYRICS_TRACK).await.unwrap().is_none());
}

/// Track info (#3): *Hotel California* hi-res (24/192, ~5,071 kbps) and *Billie Jean*
/// from "Thriller 25" (16/44.1, hi-res edition found by ISRC).
#[tokio::test]
#[ignore = "network and TIDAL session required"]
async fn live_track_info() {
    let client = client().await;
    let start = std::time::Instant::now();
    let info = client.track_info(TrackId(68_709_105)).await.unwrap();
    println!("{info:#?}\nin {:?}", start.elapsed());
    let stream = info.stream.as_ref().unwrap();
    assert_eq!(stream.bit_depth, Some(24));
    assert_eq!(stream.sample_rate, Some(192_000));
    assert!(
        stream.bitrate.unwrap() > 2_000,
        "FLAC 24/192: several Mbit/s"
    );
    assert!(info.track_gain.is_some() && info.track_peak.is_some());
    assert!(info.tags.iter().any(|t| t == "HIRES_LOSSLESS"));
    assert!(info.hires_edition.is_none(), "already hi-res");

    let start = std::time::Instant::now();
    let info = client.track_info(TrackId(1_632_061)).await.unwrap();
    println!("{info:#?}\nin {:?}", start.elapsed());
    assert_eq!(info.stream.as_ref().unwrap().bit_depth, Some(16));
    let edition = info.hires_edition.unwrap();
    assert_eq!(edition.stream.unwrap().bit_depth, Some(24));

    // Format of the keys (sharps, flats) on a few tracks.
    let album = client.album(ALBUM_ID).await.unwrap();
    for track in album.tracks.iter().take(8) {
        let info = client.track_info(track.id).await.unwrap();
        println!(
            "{}: {:?} {:?} {:?} bpm",
            track.title, info.key, info.key_scale, info.bpm
        );
    }
}

/// Credits (#84): rock (many roles, engineers without an artist), classical (conductor,
/// orchestra), instrumental (composer alone), track without credits. A contributor with an
/// `artistId` must open an artist page.
#[tokio::test]
#[ignore = "network and TIDAL session required"]
async fn live_track_credits() {
    let client = client().await;
    let roles = async |id: u64| {
        let credits = client.track_credits(TrackId(id)).await.unwrap();
        for group in &credits {
            let names: Vec<String> = group
                .contributors
                .iter()
                .map(|c| format!("{} ({:?})", c.name, c.artist_id))
                .collect();
            println!("{id} {}: {}", group.role, names.join(", "));
        }
        credits
    };
    let bohemian = roles(LYRICS_TRACK.0).await;
    assert_eq!(bohemian[0].role, "Composer", "writing first");
    let freddie = &bohemian[0].contributors[0];
    assert_eq!(freddie.artist_id, Some(9664));
    let page = client.artist(9664).await.unwrap();
    assert_eq!(page.artist.name, freddie.name);
    assert!(bohemian.iter().any(|g| g.role == "Mastering Engineer"));
    // `includeContributors=true` also gives engineers an id: each one opens a page.
    let mut ids: Vec<u64> = bohemian
        .iter()
        .flat_map(|g| g.contributors.iter().filter_map(|c| c.artist_id))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    for id in ids {
        let page = client.artist(id).await;
        assert!(page.is_ok(), "artist {id}: {page:?}");
    }

    // Beethoven, 5th symphony, Karajan and the Berlin Philharmonic.
    let karajan = roles(4_361_552).await;
    for role in ["Composer", "Conductor", "Orchestra"] {
        assert!(karajan.iter().any(|g| g.role == role), "{role} missing");
    }
    // Clair de lune (Mantovani) and Interstellar: instrumentals.
    assert!(!roles(NO_LYRICS_TRACK.0).await.is_empty());
    let interstellar = roles(156_749_594).await;
    assert_eq!(interstellar[0].contributors[0].artist_id, Some(ARTIST_ID));
    // Without credits (and a nonexistent track): empty list, no error.
    assert!(roles(21_125_042).await.is_empty());
    assert!(roles(1).await.is_empty());
}

#[tokio::test]
#[ignore = "network and TIDAL session required"]
async fn live_album_credits() {
    let client = client().await;
    let tracks = client.album(ALBUM_ID).await.unwrap().tracks.len();
    let credits = client.album_credits(ALBUM_ID).await.unwrap();
    for group in &credits {
        let names: Vec<String> = group
            .contributors
            .iter()
            .map(|c| format!("{} {:?}", c.name, c.tracks.len()))
            .collect();
        println!("{}: {}", group.role, names.join(", "));
    }
    println!("{tracks} tracks");
    let zimmer = credits
        .iter()
        .flat_map(|g| &g.contributors)
        .find(|c| c.name == "Hans Zimmer")
        .expect("Hans Zimmer credited");
    assert_eq!(zimmer.artist_id, Some(ARTIST_ID));
}

/// A contributor's page (#429): Hans Zimmer (composer, 7,605 tracks on 2026-11-18), next
/// batch, end of the catalog; Mike Stone (engineer of *Bohemian Rhapsody*).
#[tokio::test]
#[ignore = "network and TIDAL session required"]
async fn live_contributor() {
    use super::contributor::BATCH;
    let client = client().await;
    let start = std::time::Instant::now();
    let page = client.contributor(ARTIST_ID, 0).await.unwrap();
    println!(
        "{}: {} tracks out of {} in {:?}, next {:?}",
        page.artist.name,
        page.tracks.len(),
        page.total,
        start.elapsed(),
        page.next_offset
    );
    for c in page.tracks.iter().take(5) {
        println!("  {}: {}", c.track.title, c.roles.join(", "));
    }
    assert_eq!(page.artist.id, ARTIST_ID);
    assert!(page.total > 1_000);
    assert!(page.tracks.len() > 150, "later batches not read?");
    assert!(
        page.tracks
            .iter()
            .any(|c| c.roles.iter().any(|r| r == "Composer"))
    );
    assert_eq!(page.next_offset, Some(BATCH));
    let more = client.contributor(ARTIST_ID, BATCH).await.unwrap();
    assert_eq!(more.next_offset, Some(2 * BATCH));
    let first: Vec<_> = page.tracks.iter().map(|c| c.track.id).collect();
    let overlap = more
        .tracks
        .iter()
        .filter(|c| first.contains(&c.track.id))
        .count();
    println!(
        "batch 2: {} tracks, {overlap} already seen",
        more.tracks.len()
    );
    assert!(overlap < 10);
    let last = client
        .contributor(ARTIST_ID, page.total - 10)
        .await
        .unwrap();
    assert!(!last.tracks.is_empty() && last.next_offset.is_none());

    let stone = client.contributor(5_022_424, 0).await.unwrap();
    println!("{}: {} tracks", stone.artist.name, stone.total);
    assert!(
        stone
            .tracks
            .iter()
            .any(|c| c.roles.iter().any(|r| r.contains("Engineer")))
    );
    assert!(client.contributor(1, 0).await.is_err());
}

/// Extras of the artist page (#205): bio with links, similar artists, "Appears on".
#[tokio::test]
#[ignore = "network and TIDAL session required"]
async fn live_artist_extra() {
    use super::models::BioPart;
    let client = client().await;
    let start = std::time::Instant::now();
    let extra = client.artist_extra(ARTIST_ID).await;
    println!("in {:?}", start.elapsed());
    let bio = extra.bio.expect("no biography");
    let links = bio
        .paragraphs
        .iter()
        .flatten()
        .filter(|p| !matches!(p, BioPart::Text { .. }))
        .count();
    println!(
        "bio ({:?}): {} paragraphs, {links} links; {} similar, {} \"appears on\"",
        bio.source,
        bio.paragraphs.len(),
        extra.similar.len(),
        extra.appears_on.len()
    );
    assert!(links > 0, "no internal link recognized");
    assert!(!extra.similar.is_empty());
    assert!(!extra.appears_on.is_empty());
}

/// Bottom of the album page (#350): *Nevermind* (77610756) has 5 other editions
/// (2026-11-12), *Thriller 25* (1632061) answers 404 (absent from the country) and gives
/// nothing.
#[tokio::test]
#[ignore = "network and TIDAL session required"]
async fn live_album_related() {
    let client = client().await;
    for id in [77_610_756, ALBUM_ID, 20_767_338, 1_632_061] {
        let start = std::time::Instant::now();
        let related = client.album_related(id).await;
        println!("{id} in {:?}", start.elapsed());
        for a in &related.versions {
            println!("  version: {} {} (hi-res {})", a.id, a.title, a.hires);
        }
        println!("  {} other albums by the artist", related.by_artist.len());
        assert!(related.versions.iter().all(|a| a.id != id));
        assert!(related.by_artist.iter().all(|a| a.id != id));
    }
    let nevermind = client.album_related(77_610_756).await;
    assert!(nevermind.versions.len() >= 3, "versions of Nevermind");
    assert!(!nevermind.by_artist.is_empty());
}

/// Links from other services pasted into the Jam (#229): contacts Deezer, Spotify, Apple
/// and YouTube in addition to TIDAL.
#[tokio::test]
#[ignore]
async fn live_foreign_links() {
    let client = client().await;
    for link in [
        "https://tidal.com/browse/album/20115556",
        "https://www.deezer.com/fr/track/67238735",
        "https://www.deezer.com/album/6575789",
        "https://open.spotify.com/intl-fr/track/69kOkLUCkxIZYexIgSG8rq?si=abc",
        "https://open.spotify.com/album/4m2880jivSbbyEGAKfITCa",
        "https://music.apple.com/fr/album/get-lucky/617154241?i=617154366",
        "https://music.apple.com/us/album/random-access-memories/617154241",
        "https://music.youtube.com/watch?v=5NV6Rdv1a3I",
        "https://open.spotify.com/track/0VjIjW4GlUZAMYd2vXMi3b",
        "https://youtu.be/XFkzRNyygfk",
    ] {
        let started = std::time::Instant::now();
        let found = crate::foreign::resolve(&client, link)
            .await
            .unwrap()
            .unwrap();
        let first = found
            .tracks
            .first()
            .map(|t| {
                format!(
                    "track {} / {} ({})",
                    t.title, t.artists[0].name, t.album.title
                )
            })
            .or_else(|| found.albums.first().map(|a| format!("album {}", a.title)));
        println!("{link}\n  → {first:?} in {:?}", started.elapsed());
        assert!(first.is_some(), "nothing for {link}");
    }
}

/// `cargo test live_import_playlists -- --ignored --nocapture`: import of a Deezer, Spotify
/// and Apple Music playlist (#328), read only: name, tracks read, sure, doubtful, not
/// found, duration.
#[tokio::test]
#[ignore = "network and TIDAL session required"]
async fn live_import_playlists() {
    use crate::foreign::{import::match_all, playlist_ref, playlists::fetch};

    let client = client().await;
    for link in [
        "https://www.deezer.com/fr/playlist/908622995",
        "https://open.spotify.com/playlist/37i9dQZF1DX4UtSsGT1Sbe",
        "https://music.apple.com/us/playlist/todays-hits/pl.f4d106fed2bd41149aaacabb233eb5eb",
    ] {
        let started = std::time::Instant::now();
        let playlist = playlist_ref(link).await.unwrap().unwrap();
        let listing = fetch(&playlist).await.unwrap();
        let (name, total, read) = (listing.name.clone(), listing.total, listing.entries.len());
        let matched = match_all(&client, listing.entries, &|_| {}, &|| false)
            .await
            .unwrap();
        let sure = matched.iter().filter(|m| m.sure).count();
        let missing = matched.iter().filter(|m| m.track.is_none()).count();
        println!(
            "{name} ({}): {read} read out of {total}, {sure} sure, {} doubtful, {missing} not found, in {:?}",
            playlist.service(),
            read - sure - missing,
            started.elapsed()
        );
        for m in matched.iter().filter(|m| !m.sure) {
            println!(
                "  {} / {:?} → {:?}",
                m.entry.wanted.title,
                m.entry.wanted.artist,
                m.track
                    .as_ref()
                    .map(|t| format!("{} / {} ({} s)", t.title, t.artists[0].name, t.duration))
            );
        }
        assert!(
            read > 0 && sure * 2 > read,
            "too few sure tracks for {link}"
        );
    }
}

/// `cargo test live_mobile_qualities -- --ignored --nocapture`: the four tiers of the PKCE
/// client (#325, #366): manifest type, codec, format, no DRM, normalization gain. 97320484
/// is a hi-res track (24/96).
#[tokio::test]
#[ignore = "network and TIDAL session required"]
async fn live_mobile_qualities() {
    use super::models::{AudioQuality, StreamSource, StreamUse};

    let client = client().await;
    for id in [
        LYRICS_TRACK,
        NO_LYRICS_TRACK,
        TrackId(1_632_061),
        TrackId(97_320_484),
    ] {
        println!("{id}");
        let mut gains = Vec::new();
        for quality in [
            AudioQuality::Low,
            AudioQuality::High,
            AudioQuality::Lossless,
            AudioQuality::HiResLossless,
        ] {
            // `stream` refuses an encrypted manifest or initialization segment.
            let stream = client.stream(id, quality, StreamUse::Play).await.unwrap();
            let kind = match &stream.source {
                StreamSource::Dash(mpd) => {
                    let shape: String = mpd
                        .split('<')
                        .filter(|tag| !tag.starts_with('/'))
                        .map(|tag| tag.split([' ', '>']).next().unwrap_or_default())
                        .collect::<Vec<_>>()
                        .join(" ");
                    format!("MPD ({shape})")
                }
                StreamSource::Direct(_) => "direct URL (BTS)".to_owned(),
            };
            let f = &stream.format;
            println!(
                "  {:<16} → {:?} {} {:?}/{:?} {:?} kbps, gain {:?}
    {kind}",
                quality.as_api_str(),
                f.quality,
                f.codec,
                f.bit_depth,
                f.sample_rate,
                f.bitrate,
                stream.gain
            );
            match quality {
                AudioQuality::Low | AudioQuality::High => {
                    assert_eq!(f.quality, quality);
                    assert!(f.codec.starts_with("mp4a"), "AAC expected");
                }
                _ => assert!(f.codec.contains("flac"), "FLAC expected"),
            }
            gains.push(stream.gain);
        }
        assert!(
            gains.windows(2).all(|w| w[0] == w[1]),
            "same gain in all qualities"
        );
    }
}
