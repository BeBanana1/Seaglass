//! Extras of the artist page (#205): biography, similar artists, « Appears on ».
//!
//! Endpoints checked on 2026-11-05 (`cargo test live_artist_extra -- --ignored --nocapture`):
//! - `artists/{id}/bio` → `{source, lastUpdated, text}`, 404 if the artist has none. The text
//!   mixes `<br/>` and internal links `[wimpLink artistId="4082"]Ennio Morricone[/wimpLink]`
//!   (also `albumId`).
//! - `artists/{id}/similar` → artist page.
//! - `artists/{id}/albums?filter=COMPILATIONS` → albums where the artist appears, mostly
//!   compilations of other artists (664 for Hans Zimmer, 642 in the « Appears on » of
//!   `pages/artist`, which groups everything but weighs ~196 kB).
//!
//! Each part is independent: a missing bio or an error does not stop the others.

use serde::Deserialize;

use super::api::TidalClient;
use super::models::{ArtistExtra, Bio, BioPart};
use super::raw::{RawAlbumFull, RawArtistFull, RawPage};
use super::{Result, TidalError};

const SIMILAR_LIMIT: u32 = 20;
const APPEARS_ON_LIMIT: u32 = 50;

impl TidalClient {
    pub async fn artist_extra(&self, id: u64) -> ArtistExtra {
        let bio_path = format!("artists/{id}/bio");
        let similar_path = format!("artists/{id}/similar");
        let albums_path = format!("artists/{id}/albums");
        let similar_query = [("limit", SIMILAR_LIMIT.to_string())];
        let albums_query = [
            ("limit", APPEARS_ON_LIMIT.to_string()),
            ("filter", "COMPILATIONS".to_owned()),
        ];
        let (bio, similar, appears_on) = tokio::join!(
            self.get::<RawBio>(&bio_path, &[]),
            self.get::<RawPage<RawArtistFull>>(&similar_path, &similar_query),
            self.get::<RawPage<RawAlbumFull>>(&albums_path, &albums_query),
        );
        ArtistExtra {
            bio: optional(bio, &bio_path).and_then(RawBio::into_bio),
            similar: optional(similar, &similar_path)
                .map(|p| p.items.into_iter().map(Into::into).collect())
                .unwrap_or_default(),
            appears_on: optional(appears_on, &albums_path)
                .map(|p| p.items.into_iter().map(Into::into).collect())
                .unwrap_or_default(),
        }
    }
}

/// 404 = nothing to show; any other error is logged and the part stays empty.
fn optional<T>(result: Result<T>, path: &str) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(TidalError::Api { status: 404, .. }) => None,
        Err(e) => {
            tracing::warn!("{path} : {e}");
            None
        }
    }
}

#[derive(Deserialize)]
struct RawBio {
    source: Option<String>,
    #[serde(default)]
    text: String,
}

impl RawBio {
    fn into_bio(self) -> Option<Bio> {
        let paragraphs = parse_bio(&self.text);
        (!paragraphs.is_empty()).then(|| Bio {
            source: self.source.filter(|s| !s.trim().is_empty()),
            paragraphs,
        })
    }
}

/// Splits the biography into paragraphs (`<br/>`, blank lines) of segments: text, or a link
/// to an artist or an album for `[wimpLink artistId|albumId="…"]…[/wimpLink]`. Other
/// links keep their text, HTML tags are removed, common entities are
/// decoded.
pub(super) fn parse_bio(text: &str) -> Vec<Vec<BioPart>> {
    let normalized = text
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("<br>", "\n")
        .replace("\r\n", "\n");
    normalized
        .split('\n')
        .map(parse_paragraph)
        .filter(|parts| !parts.is_empty())
        .collect()
}

fn parse_paragraph(line: &str) -> Vec<BioPart> {
    let mut parts = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find("[wimpLink") {
        push_text(&mut parts, &rest[..start]);
        let after = &rest[start..];
        let Some(open_end) = after.find(']') else {
            rest = "";
            break;
        };
        let attrs = &after["[wimpLink".len()..open_end];
        let body = &after[open_end + 1..];
        let (inner, tail) = match body.find("[/wimpLink]") {
            Some(close) => (&body[..close], &body[close + "[/wimpLink]".len()..]),
            None => (body, ""),
        };
        let label = clean(inner);
        if !label.is_empty() {
            match link_target(attrs) {
                Some(("artistId", id)) => parts.push(BioPart::Artist { text: label, id }),
                Some(("albumId", id)) => parts.push(BioPart::Album { text: label, id }),
                _ => push_text(&mut parts, inner),
            }
        }
        rest = tail;
    }
    push_text(&mut parts, rest);
    // Edge spaces trimmed from the whole paragraph, not from each segment.
    if let Some(BioPart::Text { text }) = parts.first_mut() {
        *text = text.trim_start().to_owned();
    }
    if let Some(BioPart::Text { text }) = parts.last_mut() {
        *text = text.trim_end().to_owned();
    }
    parts.retain(|p| !matches!(p, BioPart::Text { text } if text.is_empty()));
    parts
}

/// `artistId="4082"` → `("artistId", 4082)`; other attribute or unreadable value → `None`.
fn link_target(attrs: &str) -> Option<(&'static str, u64)> {
    for key in ["artistId", "albumId"] {
        if let Some(i) = attrs.find(&format!("{key}=\"")) {
            let value = &attrs[i + key.len() + 2..];
            let id = value.split('"').next()?.parse().ok()?;
            return Some((key, id));
        }
    }
    None
}

fn push_text(parts: &mut Vec<BioPart>, raw: &str) {
    let text = clean_keep_edges(raw);
    if text.is_empty() {
        return;
    }
    if let Some(BioPart::Text { text: last }) = parts.last_mut() {
        last.push_str(&text);
    } else {
        parts.push(BioPart::Text { text });
    }
}

fn clean(raw: &str) -> String {
    clean_keep_edges(raw).trim().to_owned()
}

/// Tags removed, entities decoded, repeated spaces collapsed (edges kept: a text segment
/// often touches a link).
fn clean_keep_edges(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_tag = false;
    for c in raw.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if in_tag => {}
            _ => out.push(c),
        }
    }
    let out = out
        .replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    let mut collapsed = String::with_capacity(out.len());
    let mut space = false;
    for c in out.chars() {
        if c.is_whitespace() {
            if !space {
                collapsed.push(' ');
            }
            space = true;
        } else {
            collapsed.push(c);
            space = false;
        }
    }
    collapsed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> BioPart {
        BioPart::Text { text: s.to_owned() }
    }

    #[test]
    fn paragraphs_and_links() {
        let bio = parse_bio(
            "Zimmer cites [wimpLink artistId=\"4082\"]Ennio Morricone[/wimpLink] and \
             [wimpLink albumId=\"12\"]Psycho[/wimpLink].<br/><br/>Second  paragraph.<br />",
        );
        assert_eq!(
            bio,
            [
                vec![
                    text("Zimmer cites "),
                    BioPart::Artist {
                        text: "Ennio Morricone".into(),
                        id: 4082
                    },
                    text(" and "),
                    BioPart::Album {
                        text: "Psycho".into(),
                        id: 12
                    },
                    text("."),
                ],
                vec![text("Second paragraph.")],
            ]
        );
    }

    #[test]
    fn unknown_links_keep_their_text() {
        let bio = parse_bio("See [wimpLink trackId=\"9\"]this song[/wimpLink] now");
        assert_eq!(bio, [vec![text("See this song now")]]);
        let bio = parse_bio("Bad [wimpLink artistId=\"x\"]id[/wimpLink]");
        assert_eq!(bio, [vec![text("Bad id")]]);
    }

    #[test]
    fn tags_entities_and_broken_markup() {
        let bio = parse_bio("<i>Rock</i> &amp; roll&nbsp;star");
        assert_eq!(bio, [vec![text("Rock & roll star")]]);
        // Link not closed: the text stays.
        let bio = parse_bio("A [wimpLink artistId=\"1\"]B");
        assert_eq!(
            bio,
            [vec![
                text("A "),
                BioPart::Artist {
                    text: "B".into(),
                    id: 1
                }
            ]]
        );
        assert!(parse_bio(" <br/> ").is_empty());
        assert_eq!(parse_bio("Cut [wimpLink artistId="), [vec![text("Cut")]]);
    }

    #[test]
    fn empty_bio_is_none() {
        let raw: RawBio = serde_json::from_str(r#"{"source":"TiVo","text":"<br/>"}"#).unwrap();
        assert!(raw.into_bio().is_none());
        let raw: RawBio = serde_json::from_str(r#"{"source":"","text":"Hi"}"#).unwrap();
        let bio = raw.into_bio().unwrap();
        assert_eq!(bio.source, None);
    }
}
