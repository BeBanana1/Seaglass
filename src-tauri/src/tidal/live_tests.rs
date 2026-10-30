//! Tests sur l'API réelle, ignorés par défaut (réseau et session enregistrée requis).
//!
//! - `cargo test live_catalog -- --ignored --nocapture` : appelle chaque endpoint.
//! - `cargo test dump_ui_fixtures -- --ignored` : écrit `src/lib/mock/fixtures.json`
//!   (gitignoré, contient la bibliothèque du compte) pour prévisualiser l'UI hors Tauri.

use super::TidalClient;

const ALBUM_ID: u64 = 156_749_593;
const ARTIST_ID: u64 = 12_979;

async fn client() -> TidalClient {
    let client = TidalClient::new().unwrap();
    assert!(
        client.current_session().await.unwrap().is_some(),
        "pas de session"
    );
    client
}

#[tokio::test]
#[ignore = "réseau et session TIDAL requis"]
async fn live_catalog() {
    let client = client().await;
    let search = client.search("interstellar", 5).await.unwrap();
    println!(
        "recherche : {} titres, {} albums, {} artistes, {} playlists",
        search.tracks.len(),
        search.albums.len(),
        search.artists.len(),
        search.playlists.len()
    );
    let album = client.album(ALBUM_ID).await.unwrap();
    println!(
        "album : {} ({} titres, couleur {:?})",
        album.album.title,
        album.tracks.len(),
        album.album.color
    );
    assert_eq!(album.tracks.len(), 30);
    assert!(album.tracks[0].album.color.is_some());
    let artist = client.artist(ARTIST_ID).await.unwrap();
    println!(
        "artiste : {} ({} top, {} albums, {} singles)",
        artist.artist.name,
        artist.top_tracks.len(),
        artist.albums.len(),
        artist.singles.len()
    );
    let playlist = client.playlist(&search.playlists[0].uuid).await.unwrap();
    println!(
        "playlist : {} ({} titres)",
        playlist.playlist.title,
        playlist.tracks.len()
    );
    // `dateAdded` est fourni pour chaque titre de playlist (vérifié le 2026-10-30).
    assert!(playlist.tracks.iter().all(|t| t.added_at.is_some()));
    let favorites = client.favorite_tracks().await.unwrap();
    assert!(favorites.iter().all(|t| t.added_at.is_some()));
    println!(
        "bibliothèque : {} titres, {} albums, {} artistes, {} playlists",
        favorites.len(),
        client.favorite_albums().await.unwrap().len(),
        client.favorite_artists().await.unwrap().len(),
        client.library_playlists().await.unwrap().len()
    );
    let home = client.home().await.unwrap();
    for row in &home {
        println!("accueil : {} ({} éléments)", row.title, row.items.len());
    }
    assert!(!home.is_empty());
}

#[tokio::test]
#[ignore = "réseau et session TIDAL requis"]
async fn dump_ui_fixtures() {
    let client = client().await;
    let search = client.search("interstellar", 20).await.unwrap();
    let playlist = client.playlist(&search.playlists[0].uuid).await.unwrap();
    let mut tracks = client.favorite_tracks().await.unwrap();
    tracks.truncate(100);
    let fixtures = serde_json::json!({
        "search": search,
        "album": client.album(ALBUM_ID).await.unwrap(),
        "artist": client.artist(ARTIST_ID).await.unwrap(),
        "playlist": playlist,
        "home": client.home().await.unwrap(),
        "library": {
            "tracks": tracks,
            "albums": client.favorite_albums().await.unwrap(),
            "artists": client.favorite_artists().await.unwrap(),
            "playlists": client.library_playlists().await.unwrap(),
        },
    });
    let path = std::path::Path::new("../src/lib/mock/fixtures.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_string(&fixtures).unwrap()).unwrap();
}

/// `cargo test live_hires -- --ignored --nocapture` : *Billie Jean* de « Thriller 25 Super
/// Deluxe Edition » n'existe qu'en 16/44,1, la même ISRC est hi-res sur d'autres éditions.
#[tokio::test]
#[ignore = "réseau et session TIDAL requis"]
async fn live_hires() {
    use super::models::{AudioQuality, TrackId};

    let client = client().await;
    let track = client.track(TrackId(1_632_061)).await.unwrap();
    assert!(!track.hires);
    assert_eq!(track.isrc.as_deref(), Some("USSM19902991"));
    let start = std::time::Instant::now();
    let edition = client.hires_edition(&track).await.unwrap().unwrap();
    println!(
        "édition hi-res : {} ({}) en {:?}",
        edition.id,
        edition.album.title,
        start.elapsed()
    );
    assert!(edition.hires);
    assert!(client.known_hires_edition(track.id).is_some());
    let stream = client
        .stream(edition.id, AudioQuality::HiResLossless)
        .await
        .unwrap();
    println!("flux : {:?}", stream.format);
    assert_eq!(stream.format.bit_depth, Some(24));
    // Un titre déjà hi-res n'est jamais remplacé.
    assert!(client.hires_edition(&edition).await.unwrap().is_none());
}
