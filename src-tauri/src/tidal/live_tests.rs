//! Tests sur l'API réelle, ignorés par défaut (réseau et session enregistrée requis).
//!
//! - `cargo test live_catalog -- --ignored --nocapture` : appelle chaque endpoint.
//! - `cargo test live_lyrics -- --ignored --nocapture` : paroles synchronisées et absentes.
//! - `cargo test live_track_info -- --ignored --nocapture` : infos détaillées d'un titre (#3).
//! - `cargo test live_track_credits -- --ignored --nocapture` : crédits d'un titre (#84).
//! - `cargo test live_album_credits -- --ignored --nocapture` : crédits d'un album (#206).
//! - `cargo test live_contributor -- --ignored --nocapture` : page d'un contributeur (#429).
//! - `cargo test live_album_related -- --ignored --nocapture` : autres versions d'un album (#350).
//! - `cargo test live_hires_twin_album -- --ignored --nocapture` : album jumeau hi-res (#59).
//! - `cargo test live_locale -- --ignored --nocapture` : titres de l'accueil en anglais (#7).
//! - `cargo test live_home_pages -- --ignored --nocapture` : toutes les pages de l'accueil (#207).
//! - `cargo test live_blocks -- --ignored --nocapture` : blocages, dans `live_blocks.rs`
//!   (**modifie le compte**, puis le remet en état).
//! - `cargo test live_cleanup -- --ignored --nocapture` : nettoyage des listes (#382), dans
//!   `live_cleanup.rs` (`live_cleanup_apply` **crée puis supprime une playlist**).
//! - `cargo test live_collection -- --ignored --nocapture` : écritures dans la bibliothèque,
//!   dans `live_collection.rs` (**modifie le compte**).
//! - `cargo test live_feed_coverage -- --ignored --nocapture` : couverture du fil d'actualité
//!   et sorties ajoutées (#415), dans `live_feed.rs`.
//! - `cargo test live_folders -- --ignored --nocapture` : dossiers de playlists (#331), dans
//!   `live_folders.rs` (**crée puis supprime un dossier et deux playlists**).
//! - `cargo test dump_ui_fixtures -- --ignored` : écrit `src/lib/mock/fixtures.json`
//!   (gitignoré, contient la bibliothèque du compte) pour prévisualiser l'UI hors Tauri.

use super::TidalClient;
use super::models::{LyricsText, MediaItem, RadioSeed, TrackId};

pub(super) const ALBUM_ID: u64 = 156_749_593;
const ARTIST_ID: u64 = 12_979;
/// « Bohemian Rhapsody » : paroles horodatées (Musixmatch).
const LYRICS_TRACK: TrackId = TrackId(534_050_211);
/// « Clair de lune » : instrumental, `tracks/{id}/lyrics` répond 404.
const NO_LYRICS_TRACK: TrackId = TrackId(5_940_831);

pub(super) async fn client() -> TidalClient {
    let client = TidalClient::new().unwrap();
    assert!(
        client.current_session().await.unwrap().is_some(),
        "pas de session"
    );
    client
}

/// L'accueil suit `page.cursor` : plus de lignes que la première page (11 le 2026-11-05),
/// clés uniques.
#[tokio::test]
#[ignore = "réseau et session TIDAL requis"]
async fn live_home_pages() {
    let client = client().await;
    // Premier appel : rafraîchissement du token compris ; le second mesure l'accueil seul.
    client.home().await.unwrap();
    let start = std::time::Instant::now();
    let home = client.home().await.unwrap();
    println!("{} lignes en {:?}", home.len(), start.elapsed());
    for row in &home {
        println!(
            "  {} : {} ({} éléments)",
            row.key,
            row.title,
            row.items.len()
        );
    }
    let mut keys: Vec<&str> = home.iter().map(|r| r.key.as_str()).collect();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), home.len(), "clés en double");
    assert!(home.len() > 11, "seule la première page ?");
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
        "grosse playlist : {} ({} titres sur {:?}) en {:?}",
        page.playlist.title,
        page.tracks.len(),
        big.number_of_tracks,
        started.elapsed()
    );
    let started = std::time::Instant::now();
    let liked = client.favorite_tracks().await.unwrap();
    // 424 titres sur le compte de test : liste complète (le total compte aussi les indisponibles).
    assert!(!liked.truncated);
    println!("total annoncé : {:?}", liked.total);
    let favorites = liked.items;
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
        client.favorite_albums().await.unwrap().items.len(),
        client.favorite_artists().await.unwrap().items.len(),
        client.library_playlists().await.unwrap().items.len()
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

/// `cargo test live_hires -- --ignored --nocapture` : *Billie Jean* de « Thriller 25 Super
/// Deluxe Edition » n'existe qu'en 16/44,1, la même ISRC est hi-res sur d'autres éditions.
#[tokio::test]
#[ignore = "réseau et session TIDAL requis"]
async fn live_hires() {
    use super::models::{AudioQuality, StreamUse, TrackId};

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
        .stream(edition.id, AudioQuality::HiResLossless, StreamUse::Play)
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
    use super::models::{AudioQuality, StreamUse};

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
        .stream(edition.id, AudioQuality::HiResLossless, StreamUse::Play)
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

/// Crédits (#84) : rock (rôles nombreux, ingénieurs sans artiste), classique (chef,
/// orchestre), instrumental (compositeur seul), titre sans crédits. Un contributeur avec
/// `artistId` doit ouvrir une page d'artiste.
#[tokio::test]
#[ignore = "réseau et session TIDAL requis"]
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
            println!("{id} {} : {}", group.role, names.join(", "));
        }
        credits
    };
    let bohemian = roles(LYRICS_TRACK.0).await;
    assert_eq!(bohemian[0].role, "Composer", "écriture en premier");
    let freddie = &bohemian[0].contributors[0];
    assert_eq!(freddie.artist_id, Some(9664));
    let page = client.artist(9664).await.unwrap();
    assert_eq!(page.artist.name, freddie.name);
    assert!(bohemian.iter().any(|g| g.role == "Mastering Engineer"));
    // `includeContributors=true` donne aussi un id aux ingénieurs : chacun ouvre une page.
    let mut ids: Vec<u64> = bohemian
        .iter()
        .flat_map(|g| g.contributors.iter().filter_map(|c| c.artist_id))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    for id in ids {
        let page = client.artist(id).await;
        assert!(page.is_ok(), "artiste {id} : {page:?}");
    }

    // Beethoven, 5e symphonie, Karajan et le Philharmonique de Berlin.
    let karajan = roles(4_361_552).await;
    for role in ["Composer", "Conductor", "Orchestra"] {
        assert!(karajan.iter().any(|g| g.role == role), "{role} absent");
    }
    // Clair de lune (Mantovani) et Interstellar : instrumentaux.
    assert!(!roles(NO_LYRICS_TRACK.0).await.is_empty());
    let interstellar = roles(156_749_594).await;
    assert_eq!(interstellar[0].contributors[0].artist_id, Some(ARTIST_ID));
    // Sans crédits (et titre inexistant) : liste vide, pas d'erreur.
    assert!(roles(21_125_042).await.is_empty());
    assert!(roles(1).await.is_empty());
}

#[tokio::test]
#[ignore = "réseau et session TIDAL requis"]
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
        println!("{} : {}", group.role, names.join(", "));
    }
    println!("{tracks} titres");
    let zimmer = credits
        .iter()
        .flat_map(|g| &g.contributors)
        .find(|c| c.name == "Hans Zimmer")
        .expect("Hans Zimmer crédité");
    assert_eq!(zimmer.artist_id, Some(ARTIST_ID));
}

/// Page d'un contributeur (#429) : Hans Zimmer (compositeur, 7 605 titres le 2026-11-18),
/// lot suivant, fin du catalogue ; Mike Stone (ingénieur de *Bohemian Rhapsody*).
#[tokio::test]
#[ignore = "réseau et session TIDAL requis"]
async fn live_contributor() {
    use super::contributor::BATCH;
    let client = client().await;
    let start = std::time::Instant::now();
    let page = client.contributor(ARTIST_ID, 0).await.unwrap();
    println!(
        "{} : {} titres sur {} en {:?}, suite {:?}",
        page.artist.name,
        page.tracks.len(),
        page.total,
        start.elapsed(),
        page.next_offset
    );
    for c in page.tracks.iter().take(5) {
        println!("  {} : {}", c.track.title, c.roles.join(", "));
    }
    assert_eq!(page.artist.id, ARTIST_ID);
    assert!(page.total > 1_000);
    assert!(page.tracks.len() > 150, "lots suivants non lus ?");
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
    println!("lot 2 : {} titres, {overlap} déjà vus", more.tracks.len());
    assert!(overlap < 10);
    let last = client
        .contributor(ARTIST_ID, page.total - 10)
        .await
        .unwrap();
    assert!(!last.tracks.is_empty() && last.next_offset.is_none());

    let stone = client.contributor(5_022_424, 0).await.unwrap();
    println!("{} : {} titres", stone.artist.name, stone.total);
    assert!(
        stone
            .tracks
            .iter()
            .any(|c| c.roles.iter().any(|r| r.contains("Engineer")))
    );
    assert!(client.contributor(1, 0).await.is_err());
}

/// Compléments de la page d'artiste (#205) : bio avec liens, similaires, « Apparaît sur ».
#[tokio::test]
#[ignore = "réseau et session TIDAL requis"]
async fn live_artist_extra() {
    use super::models::BioPart;
    let client = client().await;
    let start = std::time::Instant::now();
    let extra = client.artist_extra(ARTIST_ID).await;
    println!("en {:?}", start.elapsed());
    let bio = extra.bio.expect("pas de biographie");
    let links = bio
        .paragraphs
        .iter()
        .flatten()
        .filter(|p| !matches!(p, BioPart::Text { .. }))
        .count();
    println!(
        "bio ({:?}) : {} paragraphes, {links} liens ; {} similaires, {} « apparaît sur »",
        bio.source,
        bio.paragraphs.len(),
        extra.similar.len(),
        extra.appears_on.len()
    );
    assert!(links > 0, "aucun lien interne reconnu");
    assert!(!extra.similar.is_empty());
    assert!(!extra.appears_on.is_empty());
}

/// Bas de la page d'album (#350) : *Nevermind* (77610756) a 5 autres éditions (2026-11-12),
/// *Thriller 25* (1632061) répond 404 (absent du pays) et ne donne rien.
#[tokio::test]
#[ignore = "réseau et session TIDAL requis"]
async fn live_album_related() {
    let client = client().await;
    for id in [77_610_756, ALBUM_ID, 20_767_338, 1_632_061] {
        let start = std::time::Instant::now();
        let related = client.album_related(id).await;
        println!("{id} en {:?}", start.elapsed());
        for a in &related.versions {
            println!("  version : {} {} (hi-res {})", a.id, a.title, a.hires);
        }
        println!("  {} autres albums de l'artiste", related.by_artist.len());
        assert!(related.versions.iter().all(|a| a.id != id));
        assert!(related.by_artist.iter().all(|a| a.id != id));
    }
    let nevermind = client.album_related(77_610_756).await;
    assert!(nevermind.versions.len() >= 3, "versions de Nevermind");
    assert!(!nevermind.by_artist.is_empty());
}

/// Liens d'autres services collés dans la Jam (#229) : contacte Deezer, Spotify, Apple et
/// YouTube en plus de TIDAL.
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
                    "titre {} / {} ({})",
                    t.title, t.artists[0].name, t.album.title
                )
            })
            .or_else(|| found.albums.first().map(|a| format!("album {}", a.title)));
        println!("{link}\n  → {first:?} en {:?}", started.elapsed());
        assert!(first.is_some(), "rien pour {link}");
    }
}

/// `cargo test live_import_playlists -- --ignored --nocapture` : import d'une playlist
/// Deezer, Spotify et Apple Music (#328), lecture seule : nom, titres lus, sûrs, douteux,
/// introuvables, durée.
#[tokio::test]
#[ignore = "réseau et session TIDAL requis"]
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
            "{name} ({}) : {read} lus sur {total}, {sure} sûrs, {} douteux, {missing} introuvables, en {:?}",
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
            "trop peu de titres sûrs pour {link}"
        );
    }
}

/// `cargo test live_mobile_qualities -- --ignored --nocapture` : les quatre paliers du client
/// PKCE (#325, #366) : type de manifeste, codec, format, absence de DRM, gain de
/// normalisation. 97320484 est un titre hi-res (24/96).
#[tokio::test]
#[ignore = "réseau et session TIDAL requis"]
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
            // `stream` refuse un manifeste ou un segment d'initialisation chiffré.
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
                StreamSource::Direct(_) => "URL directe (BTS)".to_owned(),
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
                    assert!(f.codec.starts_with("mp4a"), "AAC attendu");
                }
                _ => assert!(f.codec.contains("flac"), "FLAC attendu"),
            }
            gains.push(stream.gain);
        }
        assert!(
            gains.windows(2).all(|w| w[0] == w[1]),
            "même gain dans toutes les qualités"
        );
    }
}
