//! Réponse libre d'un blind test (#434) comparée au titre et aux artistes. Logique pure.
//!
//! Les deux côtés sont ramenés à des mots en minuscules sans accents ni ponctuation ; le
//! titre perd ce qui est entre parenthèses, « feat. … » et un suffixe de version, l'artiste
//! un « The » initial. Une faute de frappe est tolérée selon la longueur attendue, et une
//! même saisie peut nommer l'artiste et le titre (« daft punk one more time »).

use crate::foreign::strip_decorations;
use crate::tidal::models::Track;

/// Ce que la réponse a trouvé.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Verdict {
    pub title: bool,
    pub artist: bool,
}

/// Compare `guess` au titre et aux artistes de `track`.
pub fn check(guess: &str, track: &Track) -> Verdict {
    let guess = normalize(guess);
    if guess.is_empty() {
        return Verdict::default();
    }
    let title = title_key(&track.title);
    let artists: Vec<String> = track
        .artists
        .iter()
        .map(|a| artist_key(&a.name))
        .filter(|a| !a.is_empty())
        .collect();

    // Saisie entière : un titre éponyme (« Weezer » de Weezer) compte des deux côtés.
    let whole_title = close(&guess, &title);
    let whole_artist = artists.iter().any(|a| close(&guess, a));
    if whole_title || whole_artist {
        return Verdict {
            title: whole_title,
            artist: whole_artist,
        };
    }

    // Sinon l'artiste d'abord, retiré de la saisie : « daft punk » ne trouve pas le titre
    // « Punk ».
    let mut words: Vec<&str> = guess.split(' ').collect();
    let mut artist = false;
    for name in &artists {
        if let Some((start, end)) = find(&words, name) {
            words.drain(start..end);
            artist = true;
            break;
        }
    }
    let title = find(&words, &title).is_some();
    Verdict { title, artist }
}

/// Clé d'un titre : sans décorations (« (Remastered) », « feat. … », « - Radio Edit »).
pub fn title_key(title: &str) -> String {
    let key = normalize(&strip_decorations(title));
    if key.is_empty() {
        normalize(title)
    } else {
        key
    }
}

/// Clé d'un artiste : sans « The » initial (« The Beatles » = « Beatles »).
fn artist_key(name: &str) -> String {
    let key = normalize(name);
    match key.strip_prefix("the ") {
        Some(rest) => rest.to_owned(),
        None => key,
    }
}

/// Minuscules, sans accents ni ponctuation, espaces réduits ; « & » vaut « and », et « et »
/// aussi (« Simon & Garfunkel », « Simon et Garfunkel »). Apostrophes retirées : « don't »
/// = « dont ».
pub fn normalize(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        match c {
            '\'' | '\u{2019}' => {}
            '&' => folded.push_str(" and "),
            c => fold(c, &mut folded),
        }
    }
    folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| if w == "et" { "and" } else { w })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Lettre sans diacritique (latin courant) ; les autres caractères restent tels quels.
fn fold(c: char, out: &mut String) {
    let base = match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'č' => 'c',
        'ď' | 'đ' => 'd',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' | 'ě' => 'e',
        'ğ' => 'g',
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'į' | 'ı' => 'i',
        'ł' => 'l',
        'ñ' | 'ń' | 'ň' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => 'o',
        'ř' => 'r',
        'ś' | 'š' | 'ş' => 's',
        'ť' | 'ţ' => 't',
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' | 'ų' => 'u',
        'ý' | 'ÿ' => 'y',
        'ź' | 'ż' | 'ž' => 'z',
        'œ' => return out.push_str("oe"),
        'æ' => return out.push_str("ae"),
        'ß' => return out.push_str("ss"),
        c => c,
    };
    out.push(base);
}

/// Fautes tolérées pour une clé de `len` caractères : aucune sous 4.
fn tolerance(len: usize) -> usize {
    match len {
        0..=3 => 0,
        4..=6 => 1,
        7..=12 => 2,
        _ => 3,
    }
}

fn close(guess: &str, key: &str) -> bool {
    !key.is_empty() && distance(guess, key) <= tolerance(key.chars().count())
}

/// Mots consécutifs de `words` proches de `key` (un mot de plus ou de moins toléré) :
/// leurs bornes.
fn find(words: &[&str], key: &str) -> Option<(usize, usize)> {
    if key.is_empty() || words.is_empty() {
        return None;
    }
    let n = key.split(' ').count();
    let mut sizes = vec![n, n + 1];
    if n > 1 {
        sizes.push(n - 1);
    }
    for size in sizes {
        if size > words.len() {
            continue;
        }
        for start in 0..=words.len() - size {
            if close(&words[start..start + size].join(" "), key) {
                return Some((start, start + size));
            }
        }
    }
    None
}

/// Distance de Levenshtein, en caractères.
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = if ca == *cb {
                diagonal
            } else {
                1 + diagonal.min(above).min(row[j])
            };
            diagonal = above;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::{AlbumRef, ArtistRef, TrackId};

    fn track(title: &str, artists: &[&str]) -> Track {
        Track {
            id: TrackId(1),
            title: title.into(),
            version: None,
            duration: 200,
            artists: artists
                .iter()
                .enumerate()
                .map(|(i, name)| ArtistRef {
                    id: i as u64,
                    name: (*name).into(),
                })
                .collect(),
            album: AlbumRef {
                id: 1,
                title: String::new(),
                cover_url: None,
                cover_small_url: None,
                cover_large_url: None,
                color: None,
            },
            explicit: false,
            ai: false,
            hires: false,
            isrc: None,
        }
    }

    fn found(guess: &str, t: &Track) -> (bool, bool) {
        let v = check(guess, t);
        (v.title, v.artist)
    }

    #[test]
    fn case_accents_and_punctuation_are_ignored() {
        let t = track("Je t'aime… moi non plus", &["Serge Gainsbourg"]);
        assert_eq!(found("je taime moi non plus", &t), (true, false));
        assert_eq!(found("SERGE GAINSBOURG", &t), (false, true));
        let t = track("Déjà vu", &["Beyoncé"]);
        assert_eq!(found("deja vu", &t), (true, false));
        assert_eq!(found("beyonce", &t), (false, true));
    }

    #[test]
    fn decorations_are_not_required() {
        let t = track("Bohemian Rhapsody (Remastered 2011)", &["Queen"]);
        assert_eq!(found("bohemian rhapsody", &t), (true, false));
        let t = track("Lose Yourself - Radio Edit", &["Eminem"]);
        assert_eq!(found("lose yourself", &t), (true, false));
    }

    #[test]
    fn typos_are_tolerated_by_length() {
        let t = track("Bohemian Rhapsody", &["Queen"]);
        assert_eq!(found("bohemian rapsody", &t), (true, false));
        assert_eq!(found("bohemain rhapsody", &t), (true, false));
        assert_eq!(
            found("bohemain rapsodie", &t),
            (false, false),
            "trop de fautes"
        );
        assert_eq!(found("rhapsody in blue", &t), (false, false));
        // Clé courte : aucune faute.
        let t = track("One", &["U2"]);
        assert_eq!(found("u3", &t), (false, false));
        assert_eq!(found("u2", &t), (false, true));
    }

    #[test]
    fn title_and_artist_in_one_guess() {
        let t = track("One More Time", &["Daft Punk"]);
        assert_eq!(found("daft punk one more time", &t), (true, true));
        assert_eq!(found("one more time - daft punk", &t), (true, true));
        assert_eq!(found("one more tim by daft pnk", &t), (true, true));
    }

    #[test]
    fn the_artist_does_not_count_as_the_title() {
        let t = track("Punk", &["Daft Punk"]);
        assert_eq!(found("daft punk", &t), (false, true));
        assert_eq!(found("punk", &t), (true, false));
    }

    #[test]
    fn self_titled_tracks_count_twice() {
        let t = track("Weezer", &["Weezer"]);
        assert_eq!(found("weezer", &t), (true, true));
    }

    #[test]
    fn any_artist_and_articles() {
        let t = track("Under Pressure", &["Queen", "David Bowie"]);
        assert_eq!(found("bowie", &t), (false, false));
        assert_eq!(found("david bowie", &t), (false, true));
        let t = track("Let It Be", &["The Beatles"]);
        assert_eq!(found("beatles", &t), (false, true));
        assert_eq!(found("the beatles let it be", &t), (true, true));
        let t = track("The Sound of Silence", &["Simon & Garfunkel"]);
        assert_eq!(found("simon et garfunkel", &t), (false, true));
        assert_eq!(found("simon and garfunkel", &t), (false, true));
    }

    #[test]
    fn empty_guesses_find_nothing() {
        let t = track("Hello", &["Adele"]);
        assert_eq!(found("", &t), (false, false));
        assert_eq!(found("  !!  ", &t), (false, false));
    }

    #[test]
    fn levenshtein() {
        assert_eq!(distance("", "abc"), 3);
        assert_eq!(distance("kitten", "sitting"), 3);
        assert_eq!(distance("été", "ete"), 2);
        assert_eq!(distance("same", "same"), 0);
    }
}
