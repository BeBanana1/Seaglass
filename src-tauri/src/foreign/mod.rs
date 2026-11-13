//! Liens d'autres services collés dans la recherche de la Jam (#229) : Deezer, Spotify,
//! Apple Music, YouTube Music, et TIDAL. Comportements vérifiés le 2026-11-06 (issue #229) :
//!
//! - **Deezer** : API publique `api.deezer.com/track/<id>` (ISRC) et `album/<id>` (UPC),
//!   puis `tracks?isrc=` / `albums?upc=` de TIDAL : correspondance exacte.
//! - **Spotify** : page publique (balises `og:` / `music:`) et oEmbed, sans ISRC : recherche
//!   par titre et artiste, vérifiée par la durée.
//! - **Apple Music** : API publique `itunes.apple.com/lookup`, sans ISRC : même méthode.
//! - **YouTube Music** : oEmbed, titre de la vidéo seulement : le plus fragile.
//!
//! Seuls ces hôtes sont contactés, avec le seul identifiant tiré du lien ; un lien court
//! n'est suivi que vers un lien reconnu. L'invité confirme toujours le résultat : rien
//! n'est ajouté d'office.
//!
//! Les liens de **playlist** (#328) ne servent pas à la Jam : ils ouvrent l'import
//! (`playlists.rs` lit les titres, `import.rs` cherche leurs correspondances).

pub mod import;
mod links;
mod matching;
mod meta;
pub mod playlists;

use std::sync::OnceLock;
use std::time::Duration;

use reqwest::{Client, StatusCode, redirect};
use serde::Deserialize;

pub use links::find;
use links::{Kind, Source, redirect_target};
use matching::{rank_albums, rank_editions, rank_tracks};
use meta::Wanted;
pub(crate) use meta::strip_decorations;
use playlists::PlaylistRef;

use crate::tidal::TidalClient;
use crate::tidal::models::{AlbumSummary, Track, TrackId};

const TIMEOUT: Duration = Duration::from_secs(8);
/// Redirections suivies au plus pour un lien court.
const MAX_HOPS: usize = 3;
const SEARCH_LIMIT: u32 = 30;

/// Titres ou albums TIDAL trouvés pour un lien.
#[derive(Debug, Default)]
pub struct Found {
    pub tracks: Vec<Track>,
    pub albums: Vec<AlbumSummary>,
}

fn client() -> Result<&'static Client, String> {
    static CLIENT: OnceLock<Option<Client>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            Client::builder()
                .https_only(true)
                .timeout(TIMEOUT)
                .redirect(redirect::Policy::none())
                .user_agent("Mozilla/5.0 (compatible; Seaglass)")
                .build()
                .ok()
        })
        .as_ref()
        .ok_or_else(|| "client HTTP indisponible".to_owned())
}

fn net(e: reqwest::Error) -> String {
    e.without_url().to_string()
}

/// Playlist d'un autre service désignée par le texte collé (lien court suivi) ; `None` si le
/// texte n'en contient pas.
pub async fn playlist_ref(text: &str) -> Option<Result<PlaylistRef, String>> {
    let mut source = find(text)?;
    for _ in 0..MAX_HOPS {
        let Source::Short(url) = &source else { break };
        source = match follow(url).await {
            Ok(next) => next,
            Err(e) => return Some(Err(e)),
        };
    }
    PlaylistRef::of(&source).map(Ok)
}

/// Comme `playlist_ref`, sans réseau : un lien court n'est pas suivi.
pub fn playlist_ref_offline(text: &str) -> Option<PlaylistRef> {
    PlaylistRef::of(&find(text)?)
}

/// Titres ou albums TIDAL d'un lien collé ; `None` si le texte n'en contient pas.
pub async fn resolve(tidal: &TidalClient, text: &str) -> Option<Result<Found, String>> {
    let source = find(text)?;
    Some(resolve_source(tidal, source).await)
}

async fn resolve_source(tidal: &TidalClient, mut source: Source) -> Result<Found, String> {
    for _ in 0..MAX_HOPS {
        let Source::Short(url) = &source else { break };
        source = follow(url).await?;
    }
    let terr = |e: crate::tidal::TidalError| e.to_string();
    match source {
        Source::Tidal(Kind::Track, id) => Ok(Found {
            tracks: vec![tidal.track(TrackId(id)).await.map_err(terr)?],
            ..Found::default()
        }),
        Source::Tidal(Kind::Album, id) => Ok(Found {
            albums: vec![tidal.album_summary(id).await.map_err(terr)?],
            ..Found::default()
        }),
        // Une playlist s'importe depuis l'app (#328), pas dans la Jam.
        Source::TidalOther
        | Source::Short(_)
        | Source::Tidal(Kind::Playlist, _)
        | Source::Deezer(Kind::Playlist, _)
        | Source::Spotify(Kind::Playlist, _)
        | Source::ApplePlaylist { .. }
        | Source::YouTubePlaylist => Ok(Found::default()),
        Source::Deezer(Kind::Track, id) => {
            let t: DeezerTrack = json(&format!("https://api.deezer.com/track/{id}")).await?;
            let wanted = t.wanted();
            if let Some(isrc) = t
                .isrc
                .filter(|i| i.len() == 12 && i.chars().all(|c| c.is_ascii_alphanumeric()))
            {
                let found = tidal.tracks_by_isrc(&isrc).await.map_err(terr)?;
                if !found.is_empty() {
                    return Ok(Found {
                        tracks: rank_editions(found, wanted.album.as_deref()),
                        ..Found::default()
                    });
                }
            }
            search_track(tidal, &wanted).await
        }
        Source::Deezer(Kind::Album, id) => {
            let a: DeezerAlbum = json(&format!("https://api.deezer.com/album/{id}")).await?;
            let wanted = Wanted {
                title: a.title.clone().unwrap_or_default(),
                artist: a.artist.as_ref().map(|x| x.name.clone()),
                ..Wanted::default()
            };
            if let Some(upc) = a
                .upc
                .filter(|u| (8..=14).contains(&u.len()) && u.chars().all(|c| c.is_ascii_digit()))
            {
                let found = tidal.albums_by_upc(&upc).await.map_err(terr)?;
                if !found.is_empty() {
                    return Ok(Found {
                        albums: found,
                        ..Found::default()
                    });
                }
            }
            search_album(tidal, &wanted).await
        }
        Source::Spotify(Kind::Track, id) => {
            let html = text(&format!("https://open.spotify.com/track/{id}")).await?;
            let wanted = meta::spotify_track(&html).ok_or("page Spotify illisible")?;
            search_track(tidal, &wanted).await
        }
        Source::Spotify(Kind::Album, id) => {
            let page = format!("https://open.spotify.com/album/{id}");
            let embed: OEmbed =
                json(&format!("https://open.spotify.com/oembed?url={page}")).await?;
            let html = text(&page).await.unwrap_or_default();
            search_album(tidal, &meta::spotify_album(&embed.title, &html)).await
        }
        Source::Apple { kind, id, country } => {
            let url = format!("https://itunes.apple.com/lookup?id={id}&country={country}");
            let lookup: AppleLookup = json(&url).await?;
            let item = lookup
                .results
                .into_iter()
                .next()
                .ok_or("élément Apple Music introuvable")?;
            match kind {
                Kind::Track => {
                    let wanted = Wanted {
                        title: item.track_name.unwrap_or_default(),
                        artist: item.artist_name,
                        album: item.collection_name,
                        duration: item.track_time_millis.map(|ms| (ms / 1000) as u32),
                    };
                    search_track(tidal, &wanted).await
                }
                Kind::Playlist => Ok(Found::default()),
                Kind::Album => {
                    let wanted = Wanted {
                        title: item.collection_name.unwrap_or_default(),
                        artist: item.artist_name,
                        ..Wanted::default()
                    };
                    search_album(tidal, &wanted).await
                }
            }
        }
        Source::YouTube(id) => {
            let video = format!("https://www.youtube.com/watch?v={id}");
            let embed: OEmbed = json(&format!(
                "https://www.youtube.com/oembed?url={video}&format=json"
            ))
            .await?;
            let wanted = meta::youtube(&embed.title, embed.author_name.as_deref().unwrap_or(""));
            search_track(tidal, &wanted).await
        }
    }
}

async fn search_track(tidal: &TidalClient, wanted: &Wanted) -> Result<Found, String> {
    if wanted.title.trim().is_empty() {
        return Ok(Found::default());
    }
    let results = tidal
        .search(&wanted.query(), SEARCH_LIMIT)
        .await
        .map_err(|e| e.to_string())?;
    Ok(Found {
        tracks: rank_tracks(results.tracks, wanted),
        ..Found::default()
    })
}

async fn search_album(tidal: &TidalClient, wanted: &Wanted) -> Result<Found, String> {
    if wanted.title.trim().is_empty() {
        return Ok(Found::default());
    }
    let results = tidal
        .search(&wanted.query(), SEARCH_LIMIT)
        .await
        .map_err(|e| e.to_string())?;
    Ok(Found {
        albums: rank_albums(results.albums, wanted),
        ..Found::default()
    })
}

/// Un lien court : seule une redirection vers un lien reconnu est acceptée.
async fn follow(url: &reqwest::Url) -> Result<Source, String> {
    let response = client()?.get(url.clone()).send().await.map_err(net)?;
    if !response.status().is_redirection() {
        return Err(format!(
            "lien court sans redirection ({})",
            response.status()
        ));
    }
    response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|l| l.to_str().ok())
        .and_then(redirect_target)
        .ok_or_else(|| "lien court vers une adresse inconnue".to_owned())
}

async fn get(url: &str) -> Result<reqwest::Response, String> {
    let response = client()?
        .get(url)
        .header(reqwest::header::ACCEPT_LANGUAGE, "en")
        .send()
        .await
        .map_err(net)?;
    match response.status() {
        StatusCode::OK => Ok(response),
        status => Err(format!("réponse {status}")),
    }
}

async fn text(url: &str) -> Result<String, String> {
    get(url).await?.text().await.map_err(net)
}

async fn json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, String> {
    get(url).await?.json().await.map_err(net)
}

#[derive(Deserialize)]
struct DeezerName {
    name: String,
}

#[derive(Deserialize)]
struct DeezerTitle {
    title: String,
}

#[derive(Deserialize)]
struct DeezerTrack {
    #[serde(default)]
    isrc: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    duration: Option<u32>,
    #[serde(default)]
    artist: Option<DeezerName>,
    #[serde(default)]
    album: Option<DeezerTitle>,
}

impl DeezerTrack {
    fn wanted(&self) -> Wanted {
        Wanted {
            title: self.title.clone().unwrap_or_default(),
            artist: self.artist.as_ref().map(|a| a.name.clone()),
            album: self.album.as_ref().map(|a| a.title.clone()),
            duration: self.duration,
        }
    }
}

#[derive(Deserialize)]
struct DeezerAlbum {
    #[serde(default)]
    upc: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    artist: Option<DeezerName>,
}

#[derive(Deserialize)]
struct OEmbed {
    title: String,
    #[serde(default)]
    author_name: Option<String>,
}

#[derive(Deserialize)]
struct AppleLookup {
    #[serde(default)]
    results: Vec<AppleItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppleItem {
    #[serde(default)]
    track_name: Option<String>,
    #[serde(default)]
    collection_name: Option<String>,
    #[serde(default)]
    artist_name: Option<String>,
    #[serde(default)]
    track_time_millis: Option<u64>,
}
