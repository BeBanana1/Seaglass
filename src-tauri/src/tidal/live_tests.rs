//! Tests sur l'API réelle, ignorés par défaut (réseau et session enregistrée requis).
//!
//! - `cargo test live_catalog -- --ignored --nocapture` : appelle chaque endpoint.
//! - `cargo test live_lyrics -- --ignored --nocapture` : paroles synchronisées et absentes.
//! - `cargo test dump_ui_fixtures -- --ignored` : écrit `src/lib/mock/fixtures.json`
//!   (gitignoré, contient la bibliothèque du compte) pour prévisualiser l'UI hors Tauri.

use super::TidalClient;
use super::models::{LyricsText, MediaItem, RadioSeed, TrackId};

const ALBUM_ID: u64 = 156_749_593;
const ARTIST_ID: u64 = 12_979;
/// « Bohemian Rhapsody » : paroles horodatées (Musixmatch).
const LYRICS_TRACK: TrackId = TrackId(534_050_211);
/// « Clair de lune » : instrumental, `tracks/{id}/lyrics` répond 404.
const NO_LYRICS_TRACK: TrackId = TrackId(5_940_831);

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
    let feed = client.feed().await.unwrap();
    println!(
        "fil : {} sorties, {} non vues",
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
    println!("fil : {} mixes", mixes.len());
    // Le mix mensuel du fil s'ouvre comme les autres (`pages/mix` + `mixes/{id}/items`).
    if let Some(mix) = mixes.first() {
        let page = client.mix(&mix.id).await.unwrap();
        println!(
            "mix du fil : {} ({} titres)",
            page.mix.title,
            page.tracks.len()
        );
        assert!(!page.tracks.is_empty());
    }
    let mixes = client.favorite_mixes().await.unwrap();
    println!("mixes enregistrés : {}", mixes.len());
    let home_mix = home.iter().flat_map(|r| &r.items).find_map(|i| match i {
        MediaItem::Mix(m) => Some(m.id.clone()),
        _ => None,
    });
    if let Some(id) = home_mix.or_else(|| mixes.first().map(|m| m.id.clone())) {
        let mix = client.mix(&id).await.unwrap();
        println!("mix : {} ({} titres)", mix.mix.title, mix.tracks.len());
        assert!(!mix.tracks.is_empty());
        assert!(mix.image_large_url.is_some());
    }
    let track_radio = client
        .radio(RadioSeed::Track, album.tracks[0].id.0)
        .await
        .unwrap();
    let artist_radio = client.radio(RadioSeed::Artist, ARTIST_ID).await.unwrap();
    println!(
        "radios : {} ({} titres), {} ({} titres)",
        track_radio.title,
        track_radio.tracks.len(),
        artist_radio.title,
        artist_radio.tracks.len()
    );
    assert!(!track_radio.tracks.is_empty() && !artist_radio.tracks.is_empty());
}

#[tokio::test]
#[ignore = "réseau et session TIDAL requis"]
async fn dump_ui_fixtures() {
    let client = client().await;
    let search = client.search("interstellar", 20).await.unwrap();
    let playlist = client.playlist(&search.playlists[0].uuid).await.unwrap();
    let mut tracks = client.favorite_tracks().await.unwrap();
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
            "albums": client.favorite_albums().await.unwrap(),
            "artists": client.favorite_artists().await.unwrap(),
            "playlists": client.library_playlists().await.unwrap(),
            "mixes": mixes,
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

#[tokio::test]
#[ignore = "réseau et session TIDAL requis"]
async fn live_lyrics() {
    let client = client().await;
    let lyrics = client.lyrics(LYRICS_TRACK).await.unwrap().unwrap();
    let LyricsText::Synced { lines } = &lyrics.text else {
        panic!("paroles non synchronisées");
    };
    println!(
        "paroles : {} lignes ({:?}), première à {} ms : {}",
        lines.len(),
        lyrics.provider,
        lines[0].time_ms,
        lines[0].text
    );
    assert!(lines.windows(2).all(|w| w[0].time_ms <= w[1].time_ms));
    assert!(client.lyrics(NO_LYRICS_TRACK).await.unwrap().is_none());
}

/// `cargo test live_collection -- --ignored --nocapture` : **modifie le compte**. Like puis
/// retrait d'un titre absent des favoris, puis playlist jetable créée, remplie, vidée et
/// supprimée. Le nettoyage a lieu même si une vérification échoue.
#[tokio::test]
#[ignore = "réseau et session TIDAL requis, modifie la bibliothèque"]
async fn live_collection() {
    let client = client().await;
    let album = client.album(ALBUM_ID).await.unwrap();
    let liked: Vec<TrackId> = client
        .favorite_tracks()
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.track.id)
        .collect();
    let fresh: Vec<TrackId> = album
        .tracks
        .iter()
        .map(|t| t.id)
        .filter(|id| !liked.contains(id))
        .take(4)
        .collect();
    assert_eq!(fresh.len(), 4, "pas assez de titres hors favoris");

    let likes = check_likes(&client, fresh[0]).await;
    // Retrait systématique : le titre n'était pas liké avant le test.
    let unliked = client.unlike_track(fresh[0]).await;

    let uuid = client
        .create_playlist(
            "Seaglass test (à supprimer)",
            "Créée par un test, supprimée aussitôt",
        )
        .await
        .unwrap();
    println!("playlist créée : {uuid}");
    let playlist = check_playlist(&client, &uuid, &fresh).await;
    let deleted = client.delete_playlist(&uuid).await;
    let gone = client.playlist(&uuid).await;
    println!(
        "après suppression : {:?}",
        gone.as_ref().map(|p| &p.playlist.title)
    );

    likes.unwrap();
    unliked.unwrap();
    playlist.unwrap();
    deleted.unwrap();
    assert!(gone.is_err(), "playlist encore lisible après suppression");
}

async fn favorite_ids(client: &TidalClient) -> Vec<TrackId> {
    let favorites = client.favorite_tracks().await.unwrap();
    favorites.into_iter().map(|t| t.track.id).collect()
}

async fn check_likes(client: &TidalClient, id: TrackId) -> Result<(), String> {
    client.like_track(id).await.map_err(|e| e.to_string())?;
    let after = favorite_ids(client).await;
    println!("like {id} : premier favori = {:?}", after.first());
    if after.first() != Some(&id) {
        return Err(format!("{id} absent de la tête des favoris"));
    }
    client.unlike_track(id).await.map_err(|e| e.to_string())?;
    if favorite_ids(client).await.contains(&id) {
        return Err(format!("{id} encore dans les favoris"));
    }
    println!("like retiré");
    Ok(())
}

async fn check_playlist(client: &TidalClient, uuid: &str, ids: &[TrackId]) -> Result<(), String> {
    let err = |e: super::TidalError| e.to_string();
    let tracks = |page: super::models::PlaylistPage| -> Vec<TrackId> {
        page.tracks.into_iter().map(|t| t.track.id).collect()
    };
    let mine = client.library_playlists().await.map_err(err)?;
    println!(
        "dans la bibliothèque : {:?}",
        mine.iter()
            .find(|p| p.uuid == uuid)
            .map(|p| (p.mine, &p.title))
    );

    client.add_to_playlist(uuid, &ids[..2]).await.map_err(err)?;
    let after_add = tracks(client.playlist(uuid).await.map_err(err)?);
    println!("ajout de 2 titres : {after_add:?}");
    if after_add != ids[..2] {
        return Err(format!("ajout : {after_add:?}"));
    }

    // Doublon avec onDupes=SKIP : ignoré ou refusé ?
    let dupe = client.add_to_playlist(uuid, &ids[..1]).await;
    let after_dupe = tracks(client.playlist(uuid).await.map_err(err)?);
    println!(
        "doublon : {:?}, titres {after_dupe:?}",
        dupe.as_ref().err().map(ToString::to_string)
    );

    // ETag périmé : une modification faite entre-temps doit être refusée.
    let stale = client.playlist_etag(uuid).await.map_err(err)?;
    client
        .add_to_playlist(uuid, &ids[2..3])
        .await
        .map_err(err)?;
    let form = [
        ("trackIds", ids[3].to_string()),
        ("onArtifactNotFound", "FAIL".to_owned()),
    ];
    let path = format!("playlists/{uuid}/items");
    let stale_write = client
        .write(
            reqwest::Method::POST,
            super::api::API_V1,
            &path,
            &[],
            Some(&form),
            Some(&stale),
        )
        .await;
    println!(
        "ETag périmé : {:?}",
        stale_write
            .as_ref()
            .map(|r| r.status())
            .map_err(ToString::to_string)
    );

    let before_remove = tracks(client.playlist(uuid).await.map_err(err)?);
    client.remove_from_playlist(uuid, 0).await.map_err(err)?;
    let after_remove = tracks(client.playlist(uuid).await.map_err(err)?);
    println!("avant retrait : {before_remove:?}, après retrait de l'index 0 : {after_remove:?}");
    if after_remove != before_remove[1..] {
        return Err(format!("retrait : {after_remove:?}"));
    }
    Ok(())
}
