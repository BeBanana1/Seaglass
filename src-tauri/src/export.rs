//! Library export (#330): metadata only (lists of tracks), never
//! audio. A complete `library.json`, plus one CSV per list for spreadsheets and
//! transfer tools between services. Pure logic: reading the lists and writing
//! the files are in `commands/export.rs`.

use serde::Serialize;

use crate::tidal::models::{
    AlbumSummary, ArtistSummary, LibraryList, PlaylistSummary, Track, TrackEntry,
};
use crate::utc::Utc;

/// Version of the `library.json` format, to raise if its shape changes.
const FORMAT: u32 = 1;
/// Byte order mark: without it, Excel reads the CSV as ANSI and breaks the accents.
const BOM: &str = "\u{feff}";
/// Maximum length of a playlist's title in its file name.
const NAME_MAX: usize = 60;

/// A playlist and its tracks in order.
pub struct PlaylistExport {
    pub summary: PlaylistSummary,
    pub tracks: Vec<TrackEntry>,
}

/// Everything that is exported.
pub struct Library {
    pub tracks: LibraryList<TrackEntry>,
    pub albums: LibraryList<AlbumSummary>,
    pub artists: LibraryList<ArtistSummary>,
    pub playlists: Vec<PlaylistExport>,
}

/// A file to write: name (without folder) and content.
pub struct File {
    pub name: String,
    pub content: String,
}

/// Files of the export: `library.json`, then the CSVs.
pub fn files(library: &Library, now: Utc) -> Result<Vec<File>, serde_json::Error> {
    let mut files = vec![
        File {
            name: "library.json".into(),
            content: json(library, now)?,
        },
        File {
            name: "liked-tracks.csv".into(),
            content: tracks_csv(&library.tracks.items),
        },
        File {
            name: "albums.csv".into(),
            content: albums_csv(&library.albums.items),
        },
        File {
            name: "artists.csv".into(),
            content: artists_csv(&library.artists.items),
        },
    ];
    // Numbered: two playlists can have the same title, and the number keeps the order
    // of the library.
    for (i, playlist) in library.playlists.iter().enumerate() {
        files.push(File {
            name: format!(
                "playlist-{:03}-{}.csv",
                i + 1,
                file_name(&playlist.summary.title)
            ),
            content: tracks_csv(&playlist.tracks),
        });
    }
    Ok(files)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonLibrary<'a> {
    format: u32,
    app: String,
    exported_at: String,
    liked_tracks: JsonList<JsonTrack<'a>>,
    albums: JsonList<JsonAlbum<'a>>,
    artists: JsonList<JsonArtist<'a>>,
    playlists: Vec<JsonPlaylist<'a>>,
}

#[derive(Serialize)]
struct JsonList<T> {
    /// Cut at the loading cap: the oldest ones are missing.
    truncated: bool,
    items: Vec<T>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonTrack<'a> {
    id: u64,
    isrc: Option<&'a str>,
    title: &'a str,
    version: Option<&'a str>,
    artists: Vec<&'a str>,
    album: &'a str,
    album_id: u64,
    /// Seconds.
    duration: u32,
    explicit: bool,
    /// ISO 8601 UTC.
    added_at: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonAlbum<'a> {
    id: u64,
    title: &'a str,
    artists: Vec<&'a str>,
    release_date: Option<&'a str>,
}

#[derive(Serialize)]
struct JsonArtist<'a> {
    id: u64,
    name: &'a str,
}

#[derive(Serialize)]
struct JsonPlaylist<'a> {
    uuid: &'a str,
    title: &'a str,
    description: Option<&'a str>,
    /// Created by the user (otherwise followed).
    mine: bool,
    tracks: Vec<JsonTrack<'a>>,
}

fn json(library: &Library, now: Utc) -> Result<String, serde_json::Error> {
    let doc = JsonLibrary {
        format: FORMAT,
        app: format!("Seaglass {}", env!("CARGO_PKG_VERSION")),
        exported_at: now.iso(),
        liked_tracks: JsonList {
            truncated: library.tracks.truncated,
            items: library.tracks.items.iter().map(json_track).collect(),
        },
        albums: JsonList {
            truncated: library.albums.truncated,
            items: library
                .albums
                .items
                .iter()
                .map(|a| JsonAlbum {
                    id: a.id,
                    title: &a.title,
                    artists: a.artists.iter().map(|r| r.name.as_str()).collect(),
                    release_date: a.release_date.as_deref(),
                })
                .collect(),
        },
        artists: JsonList {
            truncated: library.artists.truncated,
            items: library
                .artists
                .items
                .iter()
                .map(|a| JsonArtist {
                    id: a.id,
                    name: &a.name,
                })
                .collect(),
        },
        playlists: library
            .playlists
            .iter()
            .map(|p| JsonPlaylist {
                uuid: &p.summary.uuid,
                title: &p.summary.title,
                description: p.summary.description.as_deref(),
                mine: p.summary.mine,
                tracks: p.tracks.iter().map(json_track).collect(),
            })
            .collect(),
    };
    serde_json::to_string_pretty(&doc)
}

fn json_track(entry: &TrackEntry) -> JsonTrack<'_> {
    let t: &Track = &entry.track;
    JsonTrack {
        id: t.id.0,
        isrc: t.isrc.as_deref(),
        title: &t.title,
        version: t.version.as_deref(),
        artists: t.artists.iter().map(|a| a.name.as_str()).collect(),
        album: &t.album.title,
        album_id: t.album.id,
        duration: t.duration,
        explicit: t.explicit,
        added_at: entry.added_at.map(iso_ms),
    }
}

/// Milliseconds since the epoch → ISO 8601 UTC.
fn iso_ms(ms: i64) -> String {
    Utc::from_secs(ms.div_euclid(1000)).iso()
}

fn tracks_csv(entries: &[TrackEntry]) -> String {
    let header = [
        "tidal_id",
        "isrc",
        "title",
        "version",
        "artists",
        "album",
        "duration_seconds",
        "added_at",
    ];
    csv(
        &header,
        entries.iter().map(|e| {
            let t = &e.track;
            vec![
                t.id.0.to_string(),
                t.isrc.clone().unwrap_or_default(),
                t.title.clone(),
                t.version.clone().unwrap_or_default(),
                names(t.artists.iter().map(|a| a.name.as_str())),
                t.album.title.clone(),
                t.duration.to_string(),
                e.added_at.map(iso_ms).unwrap_or_default(),
            ]
        }),
    )
}

fn albums_csv(albums: &[AlbumSummary]) -> String {
    let header = ["tidal_id", "title", "artists", "release_date"];
    csv(
        &header,
        albums.iter().map(|a| {
            vec![
                a.id.to_string(),
                a.title.clone(),
                names(a.artists.iter().map(|r| r.name.as_str())),
                a.release_date.clone().unwrap_or_default(),
            ]
        }),
    )
}

fn artists_csv(artists: &[ArtistSummary]) -> String {
    csv(
        &["tidal_id", "name"],
        artists
            .iter()
            .map(|a| vec![a.id.to_string(), a.name.clone()]),
    )
}

/// Artists separated by `; ` (the comma is often in names).
fn names<'a>(names: impl Iterator<Item = &'a str>) -> String {
    names.collect::<Vec<_>>().join("; ")
}

/// RFC 4180 CSV (commas, CRLF line endings), preceded by the BOM.
fn csv(header: &[&str], rows: impl Iterator<Item = Vec<String>>) -> String {
    let mut out = String::from(BOM);
    out.push_str(&header.join(","));
    out.push_str("\r\n");
    for row in rows {
        let cells: Vec<String> = row.iter().map(|c| cell(c)).collect();
        out.push_str(&cells.join(","));
        out.push_str("\r\n");
    }
    out
}

/// Field in quotes if it contains a comma, a quote or a line break.
fn cell(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

/// Title usable in a Windows file name: forbidden and control characters
/// replaced, length capped, no trailing dot or space.
fn file_name(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| {
            let forbidden = matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
            if c.is_control() || forbidden { '_' } else { c }
        })
        .take(NAME_MAX)
        .collect();
    let trimmed = cleaned.trim().trim_end_matches(['.', ' ']);
    if trimmed.is_empty() {
        "playlist".into()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::{AlbumRef, ArtistRef, TrackId};

    fn entry(id: u64, title: &str, artists: &[&str]) -> TrackEntry {
        TrackEntry {
            track: Track {
                id: TrackId(id),
                title: title.into(),
                version: None,
                duration: 215,
                artists: artists
                    .iter()
                    .zip(1..)
                    .map(|(n, id)| ArtistRef {
                        id,
                        name: (*n).into(),
                    })
                    .collect(),
                album: AlbumRef {
                    id: 9,
                    title: "Album, \"live\"".into(),
                    cover_url: None,
                    cover_small_url: None,
                    cover_large_url: None,
                    color: None,
                },
                explicit: false,
                ai: false,
                hires: false,
                isrc: Some("USAB12345678".into()),
            },
            added_at: Some(1_793_276_645_123),
        }
    }

    fn list<T>(items: Vec<T>) -> LibraryList<T> {
        LibraryList {
            items,
            truncated: false,
            total: None,
        }
    }

    #[test]
    fn cells_are_quoted_when_needed() {
        assert_eq!(cell("simple"), "simple");
        assert_eq!(cell("a, b"), "\"a, b\"");
        assert_eq!(cell("dit \"oui\""), "\"dit \"\"oui\"\"\"");
        assert_eq!(cell("deux\nlignes"), "\"deux\nlignes\"");
    }

    #[test]
    fn tracks_csv_has_bom_header_and_rows() {
        let csv = tracks_csv(&[entry(42, "Été", &["A", "B, C"])]);
        let mut lines = csv.split("\r\n");
        assert_eq!(
            lines.next(),
            Some("\u{feff}tidal_id,isrc,title,version,artists,album,duration_seconds,added_at")
        );
        assert_eq!(
            lines.next(),
            Some(
                "42,USAB12345678,Été,,\"A; B, C\",\"Album, \"\"live\"\"\",215,2026-10-29T12:24:05Z"
            )
        );
        assert_eq!(lines.next(), Some(""));
        assert_eq!(lines.next(), None);
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_name("Rock: 70s/80s?"), "Rock_ 70s_80s_");
        assert_eq!(file_name("  ...  "), "playlist");
        assert_eq!(file_name("fin."), "fin");
        assert_eq!(file_name(&"x".repeat(100)).len(), NAME_MAX);
    }

    #[test]
    fn files_list_json_then_csvs() {
        let summary = |title: &str| PlaylistSummary {
            uuid: "u".into(),
            title: title.into(),
            description: None,
            image_url: None,
            number_of_tracks: None,
            duration: None,
            editorial: false,
            mine: true,
        };
        let library = Library {
            tracks: list(vec![entry(1, "t", &["a"])]),
            albums: list(vec![]),
            artists: list(vec![ArtistSummary {
                id: 3,
                name: "a".into(),
                picture_url: None,
            }]),
            playlists: vec![
                PlaylistExport {
                    summary: summary("Mix"),
                    tracks: vec![entry(2, "u", &["b"])],
                },
                PlaylistExport {
                    summary: summary("Mix"),
                    tracks: vec![],
                },
            ],
        };
        let files = files(&library, Utc::from_secs(0)).expect("json");
        let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "library.json",
                "liked-tracks.csv",
                "albums.csv",
                "artists.csv",
                "playlist-001-Mix.csv",
                "playlist-002-Mix.csv",
            ]
        );
        let json: serde_json::Value = serde_json::from_str(&files[0].content).expect("valid");
        assert_eq!(json["format"], 1);
        assert_eq!(json["exportedAt"], "1970-01-01T00:00:00Z");
        assert_eq!(json["likedTracks"]["items"][0]["isrc"], "USAB12345678");
        assert_eq!(
            json["likedTracks"]["items"][0]["addedAt"],
            "2026-10-29T12:24:05Z"
        );
        assert_eq!(json["playlists"][0]["tracks"][0]["id"], 2);
        assert_eq!(json["artists"]["items"][0]["name"], "a");
    }
}
