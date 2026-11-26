//! Links from other services pasted in the Jam's search (#229): Deezer, Spotify, Apple Music,
//! YouTube Music, and TIDAL. Behavior checked on 2026-11-06 (issue #229):
//!
//! - **Deezer**: public API `api.deezer.com/track/<id>` (ISRC) and `album/<id>` (UPC), then TIDAL's
//!   `tracks?isrc=` / `albums?upc=`: exact match.
//! - **Spotify**: public page (`og:` / `music:` tags) and oEmbed, no ISRC: search by title and
//!   artist, checked against the duration.
//! - **Apple Music**: public API `itunes.apple.com/lookup`, no ISRC: same method.
//! - **YouTube Music**: oEmbed, the video title only: the most fragile.
//!
//! Only these hosts are contacted, with only the identifier taken from the link; a short link is
//! only followed to a recognized link. The guest always confirms the result: nothing is added on
//! its own.
//!
//! **Playlist** links (#328) are not for the Jam: they open the import (`playlists.rs` reads the
//! tracks, `import.rs` looks for their matches).

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

use crate::i18n::{tr, trf};
use crate::tidal::TidalClient;
use crate::tidal::models::{AlbumSummary, Track, TrackId};

const TIMEOUT: Duration = Duration::from_secs(8);
/// Redirects followed at most for a short link.
const MAX_HOPS: usize = 3;
const SEARCH_LIMIT: u32 = 30;

/// TIDAL tracks or albums found for a link.
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
        .ok_or_else(|| tr("client HTTP indisponible", "HTTP client unavailable").to_owned())
}

fn net(e: reqwest::Error) -> String {
    e.without_url().to_string()
}

/// Playlist of another service named by the pasted text (short link followed); `None` if the text
/// contains none.
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

/// Like `playlist_ref`, without the network: a short link is not followed.
pub fn playlist_ref_offline(text: &str) -> Option<PlaylistRef> {
    PlaylistRef::of(&find(text)?)
}

/// TIDAL tracks or albums of a pasted link; `None` if the text contains none.
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
        // A playlist is imported from the app (#328), not in the Jam.
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
            let wanted = meta::spotify_track(&html)
                .ok_or(tr("page Spotify illisible", "unreadable Spotify page"))?;
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
            let item = lookup.results.into_iter().next().ok_or(tr(
                "élément Apple Music introuvable",
                "Apple Music item not found",
            ))?;
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

/// A short link: only a redirect to a recognized link is accepted.
async fn follow(url: &reqwest::Url) -> Result<Source, String> {
    let response = client()?.get(url.clone()).send().await.map_err(net)?;
    if !response.status().is_redirection() {
        return Err(trf!(
            "lien court sans redirection ({})",
            "short link without a redirect ({})",
            response.status()
        ));
    }
    response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|l| l.to_str().ok())
        .and_then(redirect_target)
        .ok_or_else(|| {
            tr(
                "lien court vers une adresse inconnue",
                "short link to an unknown address",
            )
            .to_owned()
        })
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
        status => Err(trf!("réponse {}", "response {}", status)),
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
