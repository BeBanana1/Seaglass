//! Tests sur l'API réelle, ignorés par défaut (réseau et session enregistrée requis).
//!
//! - `cargo test live_catalog -- --ignored --nocapture` : appelle chaque endpoint.
//! - `cargo test live_lyrics -- --ignored --nocapture` : paroles synchronisées et absentes.
//! - `cargo test live_track_info -- --ignored --nocapture` : infos détaillées d'un titre (#3).
//! - `cargo test live_hires_twin_album -- --ignored --nocapture` : album jumeau hi-res (#59).
//! - `cargo test live_locale -- --ignored --nocapture` : titres de l'accueil en anglais (#7).
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

/// La `locale` suit la langue de l'app : mêmes lignes d'accueil (même `key`), titres traduits.
/// Change la langue globale le temps du test : à lancer seul.
#[tokio::test]
#[ignore = "réseau et session TIDAL requis, change la langue globale"]
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
        println!("{} : {:?} / {:?}", row.key, row.title, fr_title);
    }
    let mix = en.iter().flat_map(|r| &r.items).find_map(|i| match i {
        MediaItem::Mix(m) => Some(m),
        _ => None,
    });
    println!("mix : {:?}", mix.map(|m| (&m.title, &m.subtitle)));
    let translated = en
        .iter()
        .filter(|r| fr.iter().any(|f| f.key == r.key && f.title != r.title))
        .count();
    assert!(
        translated > 0,
        "aucun titre de ligne ne change avec la locale"
    );
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
    // Pagination (#90) : temps de chargement d'une grosse playlist et des favoris.
    let mut found = client.library_playlists().await.unwrap();
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
        "grosse playlist : {} ({} titres sur {:?}) en {:?}",
        page.playlist.title,
        page.tracks.len(),
        big.number_of_tracks,
        started.elapsed()
    );
    let started = std::time::Instant::now();
    let favorites = client.favorite_tracks().await.unwrap();
    println!(
        "titres likés : {} en {:?}",
        favorites.len(),
        started.elapsed()
    );
    assert!(favorites.iter().all(|t| t.added_at.is_some()));
    // Pages demandées en parallèle, remises dans l'ordre : toujours du plus récent au plus ancien.
    assert!(favorites.windows(2).all(|w| w[0].added_at >= w[1].added_at));
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

/// `cargo test live_hires_twin -- --ignored --nocapture` : *Hotel California (2013
/// Remaster)* (`20767339`, 16/44,1) a un album jumeau hi-res (`68709104`, 24/192) sous
/// d'autres ISRC (#55). Les autres titres de l'album sortent du cache.
#[tokio::test]
#[ignore = "réseau et session TIDAL requis"]
async fn live_hires_twin() {
    use super::models::AudioQuality;

    let client = client().await;
    let album = client.album(20_767_338).await.unwrap();
    let (first, second) = (&album.tracks[0], &album.tracks[1]);
    assert_eq!(first.id, TrackId(20_767_339));
    assert!(!first.hires);
    let start = std::time::Instant::now();
    let edition = client.hires_edition(first).await.unwrap().unwrap();
    println!(
        "jumeau : {} ({}, {}) en {:?}",
        edition.id,
        edition.album.title,
        edition.album.id,
        start.elapsed()
    );
    assert_eq!(edition.album.id, 68_709_104);
    assert_ne!(edition.isrc, first.isrc);
    let start = std::time::Instant::now();
    let next = client.hires_edition(second).await.unwrap().unwrap();
    println!("{} → {} en {:?}", second.title, next.id, start.elapsed());
    assert_eq!(next.album.id, 68_709_104);
    let stream = client
        .stream(edition.id, AudioQuality::HiResLossless)
        .await
        .unwrap();
    println!("flux : {:?}", stream.format);
    assert_eq!(stream.format.bit_depth, Some(24));
    assert_eq!(stream.format.sample_rate, Some(192_000));
}

/// `cargo test live_hires_twin_album -- --ignored --nocapture` : la page de « Hotel
/// California (2013 Remaster) » en 16/44,1 propose son jumeau hi-res (#59), pas l'inverse.
#[tokio::test]
#[ignore = "réseau et session TIDAL requis"]
async fn live_hires_twin_album() {
    let client = client().await;
    let start = std::time::Instant::now();
    let twin = client.hires_twin_album(20_767_338).await.unwrap().unwrap();
    println!(
        "jumeau : {} ({}) en {:?}",
        twin.id,
        twin.title,
        start.elapsed()
    );
    assert_eq!(twin.id, 68_709_104);
    assert!(twin.hires);
    assert!(client.hires_twin_album(68_709_104).await.unwrap().is_none());
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
    // `favorites/ids` contient tous les titres likés, indisponibles compris (451 contre 426
    // affichés le 2026-10-30) : la liste des favoris en est un sous-ensemble.
    let liked = client.liked_track_ids().await.unwrap();
    let favorites = favorite_ids(&client).await;
    println!(
        "likes : {} identifiants, {} favoris",
        liked.len(),
        favorites.len()
    );
    assert!(favorites.iter().all(|id| liked.contains(id)));
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
    let unliked = client.set_track_liked(fresh[0], false).await;

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
    let err = |e: super::TidalError| e.to_string();
    client.set_track_liked(id, true).await.map_err(err)?;
    if !client.liked_track_ids().await.map_err(err)?.contains(&id) {
        return Err(format!("{id} absent des likes en mémoire"));
    }
    let after = favorite_ids(client).await;
    println!("like {id} : premier favori = {:?}", after.first());
    if after.first() != Some(&id) {
        return Err(format!("{id} absent de la tête des favoris"));
    }
    client.set_track_liked(id, false).await.map_err(err)?;
    if favorite_ids(client).await.contains(&id)
        || client.liked_track_ids().await.map_err(err)?.contains(&id)
    {
        return Err(format!("{id} encore liké"));
    }
    println!("like retiré");
    Ok(())
}

async fn check_playlist(client: &TidalClient, uuid: &str, ids: &[TrackId]) -> Result<(), String> {
    let err = |e: super::TidalError| e.to_string();
    let tracks = async |uuid: &str| -> Result<Vec<TrackId>, String> {
        let page = client.playlist(uuid).await.map_err(err)?;
        Ok(page.tracks.into_iter().map(|t| t.track.id).collect())
    };
    let own = client.own_playlists().await.map_err(err)?;
    if !own.iter().any(|p| p.uuid == uuid) {
        return Err("playlist créée absente de own_playlists".into());
    }

    let added = client.add_to_playlist(uuid, &ids[..2]).await.map_err(err)?;
    let after_add = tracks(uuid).await?;
    println!("ajout de {added} titres : {after_add:?}");
    if added != 2 || after_add != ids[..2] {
        return Err(format!("ajout : {added}, {after_add:?}"));
    }

    // Doublon (onDupes=SKIP) : ignoré sans erreur, `addedItemIds` vide.
    let dupe = client.add_to_playlist(uuid, &ids[..1]).await;
    let after_dupe = tracks(uuid).await?;
    println!("doublon : {dupe:?}, titres {after_dupe:?}");
    if !matches!(dupe, Ok(0)) || after_dupe != ids[..2] {
        return Err(format!("doublon : {dupe:?}"));
    }

    // ETag périmé : une modification faite entre-temps doit être refusée (412).
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
        stale_write.as_ref().map(|r| r.status())
    );
    if !matches!(stale_write, Err(super::TidalError::Conflict(_))) {
        return Err("ETag périmé accepté".into());
    }

    let before = tracks(uuid).await?;
    // Mauvais titre attendu à cette position : refusé sans rien retirer.
    let wrong = client.remove_from_playlist(uuid, 0, before[1]).await;
    if !matches!(wrong, Err(super::TidalError::Conflict(_))) {
        return Err(format!("retrait non vérifié : {wrong:?}"));
    }
    client
        .remove_from_playlist(uuid, 0, before[0])
        .await
        .map_err(err)?;
    let after = tracks(uuid).await?;
    println!("avant retrait : {before:?}, après retrait de l'index 0 : {after:?}");
    if after != before[1..] {
        return Err(format!("retrait : {after:?}"));
    }
    Ok(())
}

/// Infos d'un titre (#3) : *Hotel California* hi-res (24/192, ~5 071 kbps) et *Billie Jean*
/// de « Thriller 25 » (16/44,1, édition hi-res trouvée par ISRC).
#[tokio::test]
#[ignore = "réseau et session TIDAL requis"]
async fn live_track_info() {
    let client = client().await;
    let start = std::time::Instant::now();
    let info = client.track_info(TrackId(68_709_105)).await.unwrap();
    println!("{info:#?}\nen {:?}", start.elapsed());
    let stream = info.stream.as_ref().unwrap();
    assert_eq!(stream.bit_depth, Some(24));
    assert_eq!(stream.sample_rate, Some(192_000));
    assert!(
        stream.bitrate.unwrap() > 2_000,
        "FLAC 24/192 : plusieurs Mbit/s"
    );
    assert!(info.track_gain.is_some() && info.track_peak.is_some());
    assert!(info.tags.iter().any(|t| t == "HIRES_LOSSLESS"));
    assert!(info.hires_edition.is_none(), "déjà hi-res");

    let start = std::time::Instant::now();
    let info = client.track_info(TrackId(1_632_061)).await.unwrap();
    println!("{info:#?}\nen {:?}", start.elapsed());
    assert_eq!(info.stream.as_ref().unwrap().bit_depth, Some(16));
    let edition = info.hires_edition.unwrap();
    assert_eq!(edition.stream.unwrap().bit_depth, Some(24));

    // Format des tonalités (dièses, bémols) sur quelques titres.
    let album = client.album(ALBUM_ID).await.unwrap();
    for track in album.tracks.iter().take(8) {
        let info = client.track_info(track.id).await.unwrap();
        println!(
            "{} : {:?} {:?} {:?} bpm",
            track.title, info.key, info.key_scale, info.bpm
        );
    }
}
