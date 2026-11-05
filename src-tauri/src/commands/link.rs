//! Liens TIDAL collés (#203) : `tidal.com/browse/<type>/<id>`, `tidal.com/<type>/<id>`,
//! `listen.tidal.com/…`, avec ou sans schéma, `/u` final ou paramètres, éventuellement au
//! milieu d'un texte de partage. Le texte n'est jamais envoyé tel quel : seuls le type et
//! un identifiant validé (numérique, uuid ou id de mix) en sortent.

use serde::Serialize;
use tauri::State;

use super::catalog::{is_mix_id, is_uuid};
use super::{AppState, CmdResult, err};
use crate::tidal::models::TrackId;

/// Un texte collé plus long n'est pas un lien.
const MAX_TEXT_LEN: usize = 2_000;

/// Page à ouvrir pour un lien.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LinkTarget {
    /// Un titre ouvre son album, titre mis en évidence (`track`).
    #[serde(rename_all = "camelCase")]
    Album {
        id: u64,
        track: Option<TrackId>,
    },
    Artist {
        id: u64,
    },
    Playlist {
        uuid: String,
    },
    Mix {
        id: String,
    },
}

/// Ce que désigne un lien, avant résolution d'un titre en album.
#[derive(Debug, Clone, PartialEq)]
enum Link {
    Track(u64),
    Album(u64),
    Artist(u64),
    Playlist(String),
    Mix(String),
}

/// Page à ouvrir pour le texte collé, ou `null` s'il ne contient pas de lien TIDAL.
#[tauri::command]
pub async fn open_link(state: State<'_, AppState>, text: String) -> CmdResult<Option<LinkTarget>> {
    let Some(link) = parse(&text) else {
        return Ok(None);
    };
    Ok(Some(match link {
        Link::Track(id) => {
            let track = state.tidal.track(TrackId(id)).await.map_err(err)?;
            LinkTarget::Album {
                id: track.album.id,
                track: Some(track.id),
            }
        }
        Link::Album(id) => LinkTarget::Album { id, track: None },
        Link::Artist(id) => LinkTarget::Artist { id },
        Link::Playlist(uuid) => LinkTarget::Playlist { uuid },
        Link::Mix(id) => LinkTarget::Mix { id },
    }))
}

fn parse(text: &str) -> Option<Link> {
    if text.len() > MAX_TEXT_LEN {
        return None;
    }
    text.split_whitespace().find_map(parse_url)
}

fn parse_url(word: &str) -> Option<Link> {
    let lower = word.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .unwrap_or(&lower);
    let (host, path) = rest.split_once('/')?;
    if !matches!(host, "tidal.com" | "www.tidal.com" | "listen.tidal.com") {
        return None;
    }
    // Les identifiants gardent leur casse (ids de mix) : on relit le chemin dans le mot.
    let path = &word[word.len() - path.len()..];
    let path = path.split(['?', '#']).next().unwrap_or("");
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    // Dernière paire valide : `album/1/track/2` désigne le titre.
    segments
        .windows(2)
        .rev()
        .find_map(|pair| link(&pair[0].to_ascii_lowercase(), pair[1]))
}

fn link(kind: &str, id: &str) -> Option<Link> {
    let number = || {
        (id.len() <= 20 && id.chars().all(|c| c.is_ascii_digit()))
            .then(|| id.parse::<u64>().ok())
            .flatten()
            .filter(|&n| n > 0)
    };
    match kind {
        "track" => number().map(Link::Track),
        "album" => number().map(Link::Album),
        "artist" => number().map(Link::Artist),
        "playlist" => is_uuid(id).then(|| Link::Playlist(id.to_ascii_lowercase())),
        "mix" => is_mix_id(id).then(|| Link::Mix(id.to_owned())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{Link, parse};

    #[test]
    fn known_forms() {
        let cases = [
            (
                "https://tidal.com/browse/track/534050211",
                Link::Track(534050211),
            ),
            (
                "https://tidal.com/track/534050211/u",
                Link::Track(534050211),
            ),
            (
                "tidal.com/browse/album/156749593?play=true",
                Link::Album(156749593),
            ),
            (
                "http://www.tidal.com/album/156749593",
                Link::Album(156749593),
            ),
            ("https://listen.tidal.com/artist/12979", Link::Artist(12979)),
            ("https://listen.tidal.com/album/1/track/2", Link::Track(2)),
            (
                "https://tidal.com/browse/playlist/4BA30155-d807-446f-9e53-b67520ccfe83",
                Link::Playlist("4ba30155-d807-446f-9e53-b67520ccfe83".into()),
            ),
            (
                "https://tidal.com/browse/mix/0017d17ec5596B20cae3eadc63b948#x",
                Link::Mix("0017d17ec5596B20cae3eadc63b948".into()),
            ),
            (
                "Écoute « Time » sur TIDAL https://tidal.com/track/12/u",
                Link::Track(12),
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(parse(text), Some(expected), "{text}");
        }
    }

    #[test]
    fn rejected() {
        for text in [
            "",
            "bonjour",
            "https://evil.com/browse/track/1",
            "https://tidal.com.evil.com/track/1",
            "https://tidal.com/browse/track/abc",
            "https://tidal.com/browse/track/0",
            "https://tidal.com/browse/playlist/../../users/1",
            "https://tidal.com/browse/mix/abc%2F..",
            "https://tidal.com/browse/video/123",
            "https://tidal.com/browse/track/99999999999999999999999",
        ] {
            assert_eq!(parse(text), None, "{text}");
        }
        assert_eq!(
            parse(&format!("https://tidal.com/track/1 {}", "x".repeat(2_000))),
            None
        );
    }
}
