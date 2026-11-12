//! Lecture des liens d'autres services (#229) : logique pure, testée. Seuls le service, le
//! type et un identifiant validé en sortent ; le reste du texte n'est jamais réutilisé.

use reqwest::Url;

use crate::commands::link::{self, Link};

/// Un texte plus long n'est pas un lien collé.
const MAX_TEXT_LEN: usize = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Track,
    Album,
    /// Playlist à importer (#328) ; la Jam ne les prend pas.
    Playlist,
}

/// Ce que désigne un lien collé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Tidal(Kind, u64),
    /// Lien TIDAL vers autre chose qu'un titre ou un album (artiste, playlist…).
    TidalOther,
    Deezer(Kind, u64),
    Spotify(Kind, String),
    /// `country` : code de la boutique (`fr`, `us`), pour l'API publique d'Apple.
    Apple {
        kind: Kind,
        id: u64,
        country: String,
    },
    YouTube(String),
    /// Playlist Apple Music (#328) : `pl.<hex>` (éditoriale) ou `pl.u-<…>` (d'un utilisateur) ;
    /// `slug` = nom dans l'adresse, sans lequel Apple redirige.
    ApplePlaylist {
        id: String,
        slug: String,
        country: String,
    },
    /// Playlist YouTube Music : reconnue pour répondre qu'elle ne peut pas être importée.
    YouTubePlaylist,
    /// Lien court à suivre (une redirection vers l'un des liens ci-dessus).
    Short(Url),
}

/// Hôtes des liens courts : leur redirection n'est suivie que vers un lien reconnu.
const SHORT_HOSTS: [&str; 4] = [
    "link.deezer.com",
    "deezer.page.link",
    "spotify.link",
    "spotify.app.link",
];

/// Premier lien reconnu d'un texte collé (texte de partage compris).
pub fn find(text: &str) -> Option<Source> {
    if text.len() > MAX_TEXT_LEN {
        return None;
    }
    if let Some(link) = link::parse(text) {
        return Some(match link {
            Link::Track(id) => Source::Tidal(Kind::Track, id),
            Link::Album(id) => Source::Tidal(Kind::Album, id),
            _ => Source::TidalOther,
        });
    }
    text.split_whitespace().find_map(parse_word)
}

/// Lien vers lequel redirige un lien court, s'il est reconnu (jamais un autre lien court).
pub fn redirect_target(location: &str) -> Option<Source> {
    match find(location)? {
        Source::Short(_) => None,
        other => Some(other),
    }
}

fn parse_word(word: &str) -> Option<Source> {
    let word = word.trim_matches(|c: char| matches!(c, '<' | '>' | '(' | ')' | '"' | '\''));
    let with_scheme = if word.starts_with("http://") || word.starts_with("https://") {
        word.to_owned()
    } else {
        format!("https://{word}")
    };
    let url = Url::parse(&with_scheme).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    let segments: Vec<&str> = url
        .path_segments()
        .map(|s| s.filter(|p| !p.is_empty()).collect())
        .unwrap_or_default();
    match host {
        "deezer.com" => deezer(&segments),
        "open.spotify.com" | "play.spotify.com" => spotify(&segments),
        "music.apple.com" => apple(&url, &segments),
        "music.youtube.com" | "youtube.com" | "m.youtube.com" => {
            let param = |name: &str| url.query_pairs().find(|(k, _)| k == name).map(|p| p.1);
            match param("v") {
                Some(v) => youtube_id(&v),
                None if segments.first() == Some(&"playlist") && param("list").is_some() => {
                    Some(Source::YouTubePlaylist)
                }
                None => None,
            }
        }
        "youtu.be" => youtube_id(segments.first()?),
        h if SHORT_HOSTS.contains(&h) && !segments.is_empty() => {
            let mut url = url;
            url.set_scheme("https").ok()?;
            Some(Source::Short(url))
        }
        _ => None,
    }
}

fn number(id: &str) -> Option<u64> {
    (id.len() <= 20 && id.chars().all(|c| c.is_ascii_digit()))
        .then(|| id.parse().ok())
        .flatten()
        .filter(|&n| n > 0)
}

fn kind(word: &str) -> Option<Kind> {
    match word {
        "track" | "song" => Some(Kind::Track),
        "album" => Some(Kind::Album),
        "playlist" => Some(Kind::Playlist),
        _ => None,
    }
}

/// `deezer.com[/fr]/track/<id>`, `…/album/<id>`, `…/playlist/<id>`.
fn deezer(segments: &[&str]) -> Option<Source> {
    segments
        .windows(2)
        .find_map(|p| Some(Source::Deezer(kind(p[0])?, number(p[1])?)))
}

/// `open.spotify.com[/intl-fr]/track/<id>` (`album`, `playlist`) : identifiant base62 de 22
/// caractères.
fn spotify(segments: &[&str]) -> Option<Source> {
    segments.windows(2).find_map(|p| {
        let id = p[1];
        let valid = id.len() == 22 && id.chars().all(|c| c.is_ascii_alphanumeric());
        let kind = kind(p[0])?;
        valid.then(|| Source::Spotify(kind, id.to_owned()))
    })
}

/// `music.apple.com/<pays>/album/<nom>/<id>[?i=<titre>]`, `…/song/<nom>/<id>`,
/// `…/playlist/<nom>/pl.<id>`.
fn apple(url: &Url, segments: &[&str]) -> Option<Source> {
    let country = segments
        .first()
        .filter(|c| c.len() == 2 && c.chars().all(|ch| ch.is_ascii_alphabetic()))?;
    if let [_, "playlist", slug, id] = segments {
        let valid_slug = !slug.is_empty()
            && slug.len() <= 200
            && slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        let valid_id = id.len() <= 64
            && id.strip_prefix("pl.").is_some_and(|rest| {
                !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            });
        return (valid_slug && valid_id).then(|| Source::ApplePlaylist {
            id: (*id).to_owned(),
            slug: slug.to_ascii_lowercase(),
            country: country.to_ascii_lowercase(),
        });
    }
    let kind = segments
        .iter()
        .find_map(|s| kind(s))
        .filter(|k| *k != Kind::Playlist)?;
    let last = number(segments.last()?)?;
    let track = url
        .query_pairs()
        .find(|(k, _)| k == "i")
        .and_then(|(_, v)| number(&v));
    let (kind, id) = match (kind, track) {
        (Kind::Album, Some(track)) => (Kind::Track, track),
        (kind, _) => (kind, last),
    };
    Some(Source::Apple {
        kind,
        id,
        country: country.to_ascii_lowercase(),
    })
}

/// Identifiant de vidéo YouTube : 11 caractères `[A-Za-z0-9_-]`.
fn youtube_id(id: &str) -> Option<Source> {
    let valid = id.len() == 11
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    valid.then(|| Source::YouTube(id.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_links() {
        assert_eq!(
            find("https://tidal.com/browse/track/28048260"),
            Some(Source::Tidal(Kind::Track, 28048260))
        );
        assert_eq!(
            find("https://tidal.com/browse/artist/8847"),
            Some(Source::TidalOther)
        );
        assert_eq!(
            find("Écoute ça https://www.deezer.com/fr/track/67238735?utm=x !"),
            Some(Source::Deezer(Kind::Track, 67238735))
        );
        assert_eq!(
            find("deezer.com/album/6575789"),
            Some(Source::Deezer(Kind::Album, 6575789))
        );
        assert_eq!(
            find("https://open.spotify.com/intl-fr/track/69kOkLUCkxIZYexIgSG8rq?si=abc"),
            Some(Source::Spotify(
                Kind::Track,
                "69kOkLUCkxIZYexIgSG8rq".into()
            ))
        );
        assert_eq!(
            find("https://open.spotify.com/album/4m2880jivSbbyEGAKfITCa"),
            Some(Source::Spotify(
                Kind::Album,
                "4m2880jivSbbyEGAKfITCa".into()
            ))
        );
        assert_eq!(
            find("https://music.apple.com/fr/album/get-lucky/617154241?i=617154366"),
            Some(Source::Apple {
                kind: Kind::Track,
                id: 617154366,
                country: "fr".into()
            })
        );
        assert_eq!(
            find("https://music.apple.com/us/album/random-access-memories/617154241"),
            Some(Source::Apple {
                kind: Kind::Album,
                id: 617154241,
                country: "us".into()
            })
        );
        assert_eq!(
            find("https://music.youtube.com/watch?v=5NV6Rdv1a3I&si=x"),
            Some(Source::YouTube("5NV6Rdv1a3I".into()))
        );
        assert_eq!(
            find("https://youtu.be/5NV6Rdv1a3I?t=3"),
            Some(Source::YouTube("5NV6Rdv1a3I".into()))
        );
        assert!(matches!(
            find("https://spotify.link/AbCdEf123"),
            Some(Source::Short(_))
        ));
        assert!(matches!(
            find("http://link.deezer.com/s/30ADZ2"),
            Some(Source::Short(u)) if u.scheme() == "https"
        ));
    }

    #[test]
    fn playlists() {
        assert_eq!(
            find("https://www.deezer.com/fr/playlist/908622995?utm=x"),
            Some(Source::Deezer(Kind::Playlist, 908622995))
        );
        assert_eq!(
            find("https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M?si=abc"),
            Some(Source::Spotify(
                Kind::Playlist,
                "37i9dQZF1DXcBWIGoYBM5M".into()
            ))
        );
        assert_eq!(
            find(
                "https://music.apple.com/us/playlist/todays-hits/pl.f4d106fed2bd41149aaacabb233eb5eb"
            ),
            Some(Source::ApplePlaylist {
                id: "pl.f4d106fed2bd41149aaacabb233eb5eb".into(),
                slug: "todays-hits".into(),
                country: "us".into()
            })
        );
        assert_eq!(
            find("https://music.apple.com/FR/playlist/ete/pl.u-AbC123"),
            Some(Source::ApplePlaylist {
                id: "pl.u-AbC123".into(),
                slug: "ete".into(),
                country: "fr".into()
            })
        );
        assert_eq!(
            find("https://music.youtube.com/playlist?list=PLFgquLnL59alCl_2TQvOiD5Vgm1hCaGSI"),
            Some(Source::YouTubePlaylist)
        );
        // Une vidéo jouée dans une playlist reste la vidéo.
        assert_eq!(
            find("https://music.youtube.com/watch?v=5NV6Rdv1a3I&list=PLx"),
            Some(Source::YouTube("5NV6Rdv1a3I".into()))
        );
        for bad in [
            "https://music.apple.com/us/playlist/x/pl.",
            "https://music.apple.com/us/playlist/x/12345",
            "https://music.apple.com/us/playlist/x%2F../pl.abc",
            "https://music.apple.com/us/playlist/pl.abc",
            "https://music.youtube.com/playlist",
        ] {
            assert_eq!(find(bad), None, "{bad}");
        }
    }

    #[test]
    fn rejected() {
        assert_eq!(find("get lucky"), None);
        assert_eq!(
            find("https://open.spotify.com/artist/4tZwfgrHOc3mvqYlEYSvVi"),
            None
        );
        assert_eq!(find("https://open.spotify.com/track/court"), None);
        assert_eq!(find("https://www.deezer.com/fr/track/abc"), None);
        assert_eq!(find("https://music.youtube.com/watch?v=trop-court"), None);
        assert_eq!(find("https://exemple.com/track/1"), None);
        assert_eq!(find("https://spotify.link/"), None);
        assert_eq!(find(&"a ".repeat(MAX_TEXT_LEN)), None);
        // Une redirection ne mène jamais à un autre lien court ni ailleurs.
        assert_eq!(redirect_target("https://spotify.link/x"), None);
        assert_eq!(redirect_target("https://exemple.com/"), None);
        assert_eq!(
            redirect_target("https://open.spotify.com/track/69kOkLUCkxIZYexIgSG8rq"),
            Some(Source::Spotify(
                Kind::Track,
                "69kOkLUCkxIZYexIgSG8rq".into()
            ))
        );
    }
}
