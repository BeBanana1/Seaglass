//! Free-text answer of a blind test (#434) compared with the title and the artists. Pure
//! logic.
//!
//! Both sides are reduced to lowercase words without accents or punctuation; the title loses
//! what is in parentheses, "feat. …" and a version suffix, the artist a leading "The". A
//! typo is tolerated depending on the expected length, and a single input can name both the
//! artist and the title ("daft punk one more time").

use crate::foreign::strip_decorations;
use crate::tidal::models::Track;

/// What the answer found.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Verdict {
    pub title: bool,
    pub artist: bool,
}

/// Compares `guess` with the title and the artists of `track`.
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

    // Whole input: a self-titled track ("Weezer" by Weezer) counts on both sides.
    let whole_title = close(&guess, &title);
    let whole_artist = artists.iter().any(|a| close(&guess, a));
    if whole_title || whole_artist {
        return Verdict {
            title: whole_title,
            artist: whole_artist,
        };
    }

    // Otherwise the artist first, removed from the input: "daft punk" does not find the title
    // "Punk".
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

/// Key of a title: without decorations ("(Remastered)", "feat. …", "- Radio Edit").
pub fn title_key(title: &str) -> String {
    let key = normalize(&strip_decorations(title));
    if key.is_empty() {
        normalize(title)
    } else {
        key
    }
}

/// Key of an artist: without a leading "The" ("The Beatles" = "Beatles").
fn artist_key(name: &str) -> String {
    let key = normalize(name);
    match key.strip_prefix("the ") {
        Some(rest) => rest.to_owned(),
        None => key,
    }
}

/// Lowercase, no accents or punctuation, collapsed spaces; "&" counts as "and", and so does
/// "et" ("Simon & Garfunkel", "Simon et Garfunkel"). Apostrophes removed: "don't" = "dont".
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

/// Letter without diacritic (common Latin); other characters are left as they are.
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

/// Typos tolerated for a key of `len` characters: none below 4.
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

/// Consecutive words of `words` close to `key` (one word more or less tolerated): their
/// bounds.
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

/// Levenshtein distance, in characters.
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
            "too many typos"
        );
        assert_eq!(found("rhapsody in blue", &t), (false, false));
        // Short key: no typo allowed.
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
