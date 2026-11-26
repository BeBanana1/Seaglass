//! What the public pages and APIs of other services say about a track or an album (#229): reading
//! `<meta>` tags, cleaning up YouTube titles. Pure logic, tested.

/// Track or album looked up on TIDAL by its name (no known ISRC or barcode).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Wanted {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// Seconds.
    pub duration: Option<u32>,
}

impl Wanted {
    /// TIDAL search query: the title without its parentheses, then the first artist.
    pub fn query(&self) -> String {
        let title = strip_decorations(&self.title);
        match self.artist.as_deref().map(first_artist) {
            Some(artist) if !artist.is_empty() => format!("{title} {artist}"),
            _ => title,
        }
    }
}

/// First artist of a list ("Daft Punk, Pharrell Williams" → "Daft Punk").
pub fn first_artist(artists: &str) -> &str {
    artists.split([',', '&']).next().unwrap_or(artists).trim()
}

/// Content of a `<meta property|name="…" content="…">` tag, with the common HTML entities decoded.
/// Read by text search: no extra dependency.
pub fn meta(html: &str, name: &str) -> Option<String> {
    let mut rest = html;
    while let Some(start) = rest.find("<meta ") {
        let tag_start = &rest[start..];
        let end = tag_start.find('>')?;
        let tag = &tag_start[..end];
        let named = tag.contains(&format!("property=\"{name}\""))
            || tag.contains(&format!("name=\"{name}\""));
        if named && let Some(content) = attribute(tag, "content") {
            return Some(decode_entities(content));
        }
        rest = &tag_start[end..];
    }
    None
}

fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let start = tag.find(&key)? + key.len();
    let len = tag[start..].find('"')?;
    Some(&tag[start..start + len])
}

fn decode_entities(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Spotify track page: `og:title` = title, `og:description` = "Artists · Album · Song · 2013",
/// `music:duration` in seconds.
pub fn spotify_track(html: &str) -> Option<Wanted> {
    let title = meta(html, "og:title")?;
    let description = meta(html, "og:description").unwrap_or_default();
    let mut parts = description.split(" · ");
    let artist =
        meta(html, "music:musician_description").or_else(|| parts.next().map(str::to_owned));
    let album = description.split(" · ").nth(1).map(str::to_owned);
    Some(Wanted {
        title,
        artist,
        album,
        duration: meta(html, "music:duration").and_then(|d| d.parse().ok()),
    })
}

/// Spotify album: title through oEmbed (the page's `og:title` depends on the language), artist at
/// the start of `og:description` ("Daft Punk · album · 2013 · 13 songs").
pub fn spotify_album(oembed_title: &str, html: &str) -> Wanted {
    let artist = meta(html, "og:description")
        .and_then(|d| d.split(" · ").next().map(str::to_owned))
        .filter(|a| !a.is_empty());
    Wanted {
        title: oembed_title.to_owned(),
        artist,
        ..Wanted::default()
    }
}

/// YouTube video (oEmbed): an "Artist - Topic" channel publishes the title alone; otherwise the
/// title is often "Artist - Title (Official Video)".
pub fn youtube(title: &str, author: &str) -> Wanted {
    if let Some(artist) = author.strip_suffix(" - Topic") {
        return Wanted {
            title: title.to_owned(),
            artist: Some(artist.to_owned()),
            ..Wanted::default()
        };
    }
    // Here the "-" separates the artist from the title: no version cut before reading it.
    let cleaned = strip_brackets(title);
    match cleaned.split_once(" - ") {
        Some((artist, song)) => Wanted {
            title: strip_decorations(song),
            artist: Some(artist.trim().to_owned()),
            ..Wanted::default()
        },
        None => Wanted {
            title: cleaned,
            artist: Some(author.trim_end_matches("VEVO").trim().to_owned())
                .filter(|a| !a.is_empty()),
            ..Wanted::default()
        },
    }
}

/// Words that, after "-", name a version and not the title ("Dreams - 2004 Remaster", "Danger Zone
/// - From "Top Gun"", Spotify's style, #328).
const VERSION_WORDS: [&str; 26] = [
    "remaster",
    "remastered",
    "remasterisé",
    "version",
    "mono",
    "stereo",
    "edit",
    "live",
    "from",
    "single",
    "demo",
    "acoustic",
    "instrumental",
    "bonus",
    "anniversary",
    "deluxe",
    "soundtrack",
    "theme",
    "mix",
    "remix",
    "take",
    "recording",
    "radio",
    "extended",
    "original",
    "mixed",
];

/// Title without what is between parentheses or brackets, nor a final "ft. …" / "feat. …", nor a
/// version suffix after "-".
pub fn strip_decorations(title: &str) -> String {
    strip_version_suffix(&strip_brackets(title))
}

/// Cuts at the first "-" followed by a version word; "Hey - You" stays whole.
fn strip_version_suffix(title: &str) -> String {
    let mut from = 0;
    while let Some(i) = title[from..].find(" - ").map(|i| i + from) {
        let suffix = title[i + 3..].to_lowercase();
        let versioned = suffix
            .split(|c: char| !c.is_alphanumeric())
            .any(|w| VERSION_WORDS.contains(&w));
        if versioned && i > 0 {
            return title[..i].trim().to_owned();
        }
        from = i + 3;
    }
    title.to_owned()
}

/// Title without what is between parentheses or brackets, nor a final "ft. …" / "feat. …".
fn strip_brackets(title: &str) -> String {
    let mut out = String::new();
    let mut depth = 0u32;
    for c in title.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    // `to_ascii_lowercase` keeps the length in bytes: the index found holds for `out` (#262), which
    // `to_lowercase` does not guarantee ("İ" grows longer, for example).
    let lower = out.to_ascii_lowercase();
    let cut = [" ft. ", " ft ", " feat. ", " feat ", " featuring "]
        .iter()
        .filter_map(|m| lower.find(m))
        .min();
    if let Some(cut) = cut
        && out.is_char_boundary(cut)
    {
        out.truncate(cut);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRACK_PAGE: &str = r#"<head><meta property="og:title" content="Get Lucky (feat. Pharrell Williams and Nile Rodgers)"/>
<meta property="og:description" content="Daft Punk, Pharrell Williams, Nile Rodgers · Random Access Memories · Song · 2013"/>
<meta name="music:duration" content="370"/>
<meta name="music:musician_description" content="Daft Punk, Pharrell Williams, Nile Rodgers"/></head>"#;

    #[test]
    fn spotify_pages() {
        let track = spotify_track(TRACK_PAGE).unwrap();
        assert_eq!(track.duration, Some(370));
        assert_eq!(track.album.as_deref(), Some("Random Access Memories"));
        assert_eq!(track.query(), "Get Lucky Daft Punk");

        let album = spotify_album(
            "Random Access Memories",
            r#"<meta property="og:description" content="Daft Punk · album · 2013 · 13 songs"/>"#,
        );
        assert_eq!(album.query(), "Random Access Memories Daft Punk");
        assert_eq!(
            meta(
                "<meta property=\"og:title\" content=\"Rock &amp; Roll\"/>",
                "og:title"
            )
            .as_deref(),
            Some("Rock & Roll")
        );
        assert_eq!(meta("<p>rien</p>", "og:title"), None);
    }

    #[test]
    fn youtube_titles() {
        let official = youtube(
            "Daft Punk - Get Lucky (Official Audio) ft. Pharrell Williams, Nile Rodgers",
            "DaftPunkVEVO",
        );
        assert_eq!(official.title, "Get Lucky");
        assert_eq!(official.artist.as_deref(), Some("Daft Punk"));

        let topic = youtube("Get Lucky (feat. Pharrell Williams)", "Daft Punk - Topic");
        assert_eq!(topic.query(), "Get Lucky Daft Punk");

        // "Live" belongs to the title here: the artist is still read before the "-".
        let live = youtube("Nirvana - Lithium Live", "NirvanaVEVO");
        assert_eq!(live.artist.as_deref(), Some("Nirvana"));
        assert_eq!(live.title, "Lithium Live");

        let bare = youtube("Creep [Remastered]", "RadioheadVEVO");
        assert_eq!(bare.query(), "Creep Radiohead");
    }

    #[test]
    fn decorations_are_removed() {
        assert_eq!(
            strip_decorations("Get Lucky (Radio Edit) [feat. X]"),
            "Get Lucky"
        );
        assert_eq!(strip_decorations("Stay feat. Justin Bieber"), "Stay");
        // Spotify-style version suffixes (#328).
        assert_eq!(strip_decorations("Dreams - 2004 Remaster"), "Dreams");
        assert_eq!(strip_decorations("Paranoid - 2012 - Remaster"), "Paranoid");
        assert_eq!(
            strip_decorations("Danger Zone - From \"Top Gun\" Original Soundtrack"),
            "Danger Zone"
        );
        assert_eq!(
            strip_decorations("Layla - 40th Anniversary Version / 2010 Remastered"),
            "Layla"
        );
        assert_eq!(strip_decorations("Hey - You"), "Hey - You");
        assert_eq!(strip_decorations("- Live"), "- Live");
        // "İ" grows longer in lowercase (2 → 3 bytes): the cut stays at the right place (#262).
        assert_eq!(strip_decorations("İstanbul feat. Ayla"), "İstanbul");
        assert_eq!(strip_decorations("İİİ Song feat. X"), "İİİ Song");
        assert_eq!(
            first_artist("Daft Punk, Pharrell Williams & Nile Rodgers"),
            "Daft Punk"
        );
    }
}
