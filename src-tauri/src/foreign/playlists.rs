//! Tracks of another service's playlist, to import it (#328). Checked on 2026-11-12:
//!
//! - **Deezer**: public API `api.deezer.com/playlist/<id>` (name, `nb_tracks`) then
//!   `…/tracks?limit=` (1000 per page at most), with each track's ISRC.
//! - **Apple Music**: public page, `serialized-server-data` JSON: title, artist, album and duration
//!   of each track (up to 250 seen, the whole list); no ISRC.
//! - **Spotify**: `embed/playlist/<id>` page (`__NEXT_DATA__`): 100 tracks at most, title, artists
//!   and duration; the public page only gives the total (`music:song_count`).
//! - **YouTube Music**: nothing without getting past the consent wall, refused.
//!
//! Reading the pages is pure logic, tested: only these fields come out of it.

use serde::Deserialize;
use serde_json::Value;

use super::links::{Kind, Source};
use super::meta::{Wanted, meta};
use super::{DeezerTrack, json, text};
use crate::i18n::tr;

/// Tracks read at most in a playlist: beyond that, the preview says so.
pub const MAX_IMPORT: usize = 500;

/// Playlist named by a link from another service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaylistRef {
    Deezer(u64),
    Spotify(String),
    Apple {
        id: String,
        slug: String,
        country: String,
    },
    /// Recognized to answer that it cannot be imported.
    YouTube,
}

impl PlaylistRef {
    pub fn of(source: &Source) -> Option<Self> {
        Some(match source {
            Source::Deezer(Kind::Playlist, id) => Self::Deezer(*id),
            Source::Spotify(Kind::Playlist, id) => Self::Spotify(id.clone()),
            Source::ApplePlaylist { id, slug, country } => Self::Apple {
                id: id.clone(),
                slug: slug.clone(),
                country: country.clone(),
            },
            Source::YouTubePlaylist => Self::YouTube,
            _ => return None,
        })
    }

    /// Clean address (no tracking parameter), read again by `import_playlist`.
    pub fn url(&self) -> String {
        match self {
            Self::Deezer(id) => format!("https://www.deezer.com/playlist/{id}"),
            Self::Spotify(id) => format!("https://open.spotify.com/playlist/{id}"),
            Self::Apple { id, slug, country } => {
                format!("https://music.apple.com/{country}/playlist/{slug}/{id}")
            }
            Self::YouTube => "https://music.youtube.com/playlist".to_owned(),
        }
    }

    pub fn service(&self) -> &'static str {
        match self {
            Self::Deezer(_) => "deezer",
            Self::Spotify(_) => "spotify",
            Self::Apple { .. } => "apple",
            Self::YouTube => "youtube",
        }
    }
}

/// A track of the original playlist.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    pub wanted: Wanted,
    pub isrc: Option<String>,
}

/// Content read from a playlist: `total` = number of tracks at the service, sometimes more than
/// `entries` (Spotify, cap).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listing {
    pub name: String,
    pub total: usize,
    pub entries: Vec<Entry>,
}

pub async fn fetch(playlist: &PlaylistRef) -> Result<Listing, String> {
    let mut listing = match playlist {
        PlaylistRef::Deezer(id) => deezer(*id).await?,
        PlaylistRef::Spotify(id) => spotify(id).await?,
        PlaylistRef::Apple { .. } => {
            apple_page(&text(&playlist.url()).await?).ok_or_else(unreadable)?
        }
        PlaylistRef::YouTube => return Err(youtube_unsupported()),
    };
    listing.total = listing.total.max(listing.entries.len());
    listing.entries.truncate(MAX_IMPORT);
    Ok(listing)
}

pub fn youtube_unsupported() -> String {
    tr(
        "Les playlists YouTube Music ne peuvent pas être importées : YouTube ne les montre pas sans compte",
        "YouTube Music playlists can't be imported: YouTube doesn't show them without an account",
    )
    .into()
}

fn unreadable() -> String {
    tr(
        "Playlist illisible : elle est peut-être privée, ou le service a changé sa page",
        "Unreadable playlist: it may be private, or the service changed its page",
    )
    .into()
}

#[derive(Deserialize)]
struct DeezerPlaylist {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    nb_tracks: Option<usize>,
}

#[derive(Deserialize)]
struct DeezerTracks {
    #[serde(default)]
    data: Vec<DeezerTrack>,
}

async fn deezer(id: u64) -> Result<Listing, String> {
    // A private or missing playlist answers 200 with `{"error": …}`: no title.
    let head: DeezerPlaylist = json(&format!("https://api.deezer.com/playlist/{id}")).await?;
    let name = head.title.ok_or_else(unreadable)?;
    let tracks: DeezerTracks = json(&format!(
        "https://api.deezer.com/playlist/{id}/tracks?index=0&limit={MAX_IMPORT}"
    ))
    .await?;
    let entries = tracks
        .data
        .into_iter()
        .map(|t| Entry {
            wanted: t.wanted(),
            isrc: t.isrc,
        })
        .collect();
    Ok(Listing {
        name,
        total: head.nb_tracks.unwrap_or(0),
        entries,
    })
}

async fn spotify(id: &str) -> Result<Listing, String> {
    let embed = text(&format!("https://open.spotify.com/embed/playlist/{id}")).await?;
    let mut listing = spotify_embed(&embed).ok_or_else(unreadable)?;
    // The embed page stops at 100: the real total is on the public page.
    if listing.entries.len() >= SPOTIFY_EMBED_MAX
        && let Ok(page) = text(&format!("https://open.spotify.com/playlist/{id}")).await
    {
        listing.total = meta(&page, "music:song_count")
            .and_then(|n| n.parse().ok())
            .unwrap_or(0);
    }
    Ok(listing)
}

/// Tracks shown at most by Spotify's embed page.
pub const SPOTIFY_EMBED_MAX: usize = 100;

/// JSON content of a `<script id="…">`.
fn script<'a>(html: &'a str, id: &str) -> Option<&'a str> {
    let at = html.find(&format!("id=\"{id}\""))?;
    let start = at + html[at..].find('>')? + 1;
    let len = html[start..].find("</script>")?;
    Some(&html[start..start + len])
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Milliseconds → rounded seconds.
fn seconds(ms: Option<u64>) -> Option<u32> {
    ms.filter(|&ms| ms > 0)
        .and_then(|ms| u32::try_from((ms + 500) / 1000).ok())
}

/// Spotify's `embed/playlist/<id>` page: `props.pageProps.state.data.entity`.
pub fn spotify_embed(html: &str) -> Option<Listing> {
    let data: Value = serde_json::from_str(script(html, "__NEXT_DATA__")?).ok()?;
    let entity = data.pointer("/props/pageProps/state/data/entity")?;
    let name = str_of(entity, "name").or_else(|| str_of(entity, "title"))?;
    let entries: Vec<Entry> = entity
        .get("trackList")?
        .as_array()?
        .iter()
        .filter_map(|t| {
            Some(Entry {
                wanted: Wanted {
                    title: str_of(t, "title")?.to_owned(),
                    artist: str_of(t, "subtitle").map(|a| a.replace('\u{a0}', " ")),
                    album: None,
                    duration: seconds(t.get("duration").and_then(Value::as_u64)),
                },
                isrc: None,
            })
        })
        .collect();
    Some(Listing {
        name: name.to_owned(),
        total: entries.len(),
        entries,
    })
}

/// Items of the Apple sections of a given type.
fn items_of<'a>(sections: &'a [Value], kind: &'a str) -> impl Iterator<Item = &'a Value> {
    sections
        .iter()
        .filter(move |s| s.get("itemKind").and_then(Value::as_str) == Some(kind))
        .filter_map(|s| s.get("items")?.as_array())
        .flatten()
}

/// Public page of an Apple Music playlist: `containerDetailHeaderLockup` sections (name,
/// `trackCount`) and `trackLockup` (tracks).
pub fn apple_page(html: &str) -> Option<Listing> {
    let data: Value = serde_json::from_str(script(html, "serialized-server-data")?).ok()?;
    let sections = data.pointer("/data/0/data/sections")?.as_array()?;
    let header = items_of(sections, "containerDetailHeaderLockup").next()?;
    let name = str_of(header, "title")?;
    let entries: Vec<Entry> = items_of(sections, "trackLockup")
        .filter_map(|t| {
            Some(Entry {
                wanted: Wanted {
                    title: str_of(t, "title")?.to_owned(),
                    artist: str_of(t, "artistName").map(str::to_owned),
                    album: t
                        .pointer("/tertiaryLinks/0")
                        .and_then(|l| str_of(l, "title"))
                        .map(str::to_owned),
                    duration: seconds(t.get("duration").and_then(Value::as_u64)),
                },
                isrc: None,
            })
        })
        .collect();
    let total = header
        .get("trackCount")
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(entries.len());
    Some(Listing {
        name: name.to_owned(),
        total,
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foreign::links::find;

    #[test]
    fn refs_and_urls() {
        let deezer = PlaylistRef::of(&find("https://www.deezer.com/fr/playlist/42?x=1").unwrap());
        assert_eq!(deezer, Some(PlaylistRef::Deezer(42)));
        assert_eq!(deezer.unwrap().url(), "https://www.deezer.com/playlist/42");
        let apple = PlaylistRef::of(
            &find("https://music.apple.com/us/playlist/todays-hits/pl.f4d1?l=fr").unwrap(),
        )
        .unwrap();
        assert_eq!(
            apple.url(),
            "https://music.apple.com/us/playlist/todays-hits/pl.f4d1"
        );
        // The clean address reads back as the same playlist.
        assert_eq!(PlaylistRef::of(&find(&apple.url()).unwrap()), Some(apple));
        assert_eq!(
            PlaylistRef::of(&find("https://www.deezer.com/track/1").unwrap()),
            None
        );
    }

    #[test]
    fn spotify_embed_page() {
        let html = r#"<html><script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{"state":{"data":{"entity":{"type":"playlist","name":"All Out 80s","trackList":[
            {"title":"Take On Me","subtitle":"a-ha","duration":225280},
            {"title":"Under Pressure - Remastered 2011","subtitle":"Queen, David Bowie","duration":236600},
            {"subtitle":"sans titre"}
        ]}}}}}}</script></html>"#;
        let listing = spotify_embed(html).unwrap();
        assert_eq!(listing.name, "All Out 80s");
        assert_eq!(listing.entries.len(), 2);
        assert_eq!(listing.entries[0].wanted.duration, Some(225));
        assert_eq!(
            listing.entries[1].wanted.artist.as_deref(),
            Some("Queen, David Bowie")
        );
        assert_eq!(listing.entries[1].wanted.query(), "Under Pressure Queen");
        assert_eq!(spotify_embed("<html></html>"), None);
    }

    #[test]
    fn apple_music_page() {
        let html = r#"<script type="application/json" id="serialized-server-data">{"data":[{"data":{"sections":[
            {"itemKind":"containerDetailHeaderLockup","items":[{"title":"Today’s Hits","trackCount":50}]},
            {"itemKind":"trackLockup","items":[
                {"title":"Choosin' Texas","artistName":"Ella Langley","duration":232226,
                 "tertiaryLinks":[{"title":"Choosin' Texas - Single"}]},
                {"title":"Sans artiste"}
            ]},
            {"itemKind":"bubbleLockup","items":[{"title":"Un artiste","artistName":"X"}]}
        ]}}]}</script>"#;
        let listing = apple_page(html).unwrap();
        assert_eq!(listing.name, "Today’s Hits");
        assert_eq!(listing.total, 50);
        assert_eq!(listing.entries.len(), 2);
        let first = &listing.entries[0].wanted;
        assert_eq!(first.artist.as_deref(), Some("Ella Langley"));
        assert_eq!(first.album.as_deref(), Some("Choosin' Texas - Single"));
        assert_eq!(first.duration, Some(232));
        assert_eq!(listing.entries[1].wanted.artist, None);
        assert_eq!(apple_page("<p>rien</p>"), None);
    }
}
