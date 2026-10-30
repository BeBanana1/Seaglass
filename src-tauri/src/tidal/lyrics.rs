//! Paroles : `tracks/{id}/lyrics` (vérifié le 2026-10-30).
//!
//! Réponse `{trackId, lyricsProvider, isRightToLeft, lyrics, subtitles}` : `subtitles` est au
//! format LRC (`[00:07.30] …`), `lyrics` le même texte sans horodatage. Un titre sans
//! paroles (instrumental, classique) répond 404, traduit ici en `None`.

use super::api::TidalClient;
use super::models::{LyricLine, Lyrics, LyricsText, TrackId};
use super::raw::RawLyrics;
use super::{Result, TidalError};

impl TidalClient {
    /// Paroles du titre, ou `None` si TIDAL n'en a pas.
    pub async fn lyrics(&self, id: TrackId) -> Result<Option<Lyrics>> {
        match self
            .get::<RawLyrics>(&format!("tracks/{id}/lyrics"), &[])
            .await
        {
            Ok(raw) => Ok(convert(id, raw)),
            Err(TidalError::Api { status: 404, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

/// Paroles horodatées si `subtitles` est lisible, sinon texte simple, sinon `None`.
fn convert(id: TrackId, raw: RawLyrics) -> Option<Lyrics> {
    let synced = raw
        .subtitles
        .as_deref()
        .map(parse_lrc)
        .filter(|lines| lines.iter().any(|l| !l.text.is_empty()))
        .map(|lines| LyricsText::Synced { lines });
    let text = synced.or_else(|| {
        let lines = plain_lines(raw.lyrics.as_deref()?);
        (!lines.is_empty()).then_some(LyricsText::Plain { lines })
    })?;
    Some(Lyrics {
        track_id: id,
        provider: raw.lyrics_provider.filter(|p| !p.trim().is_empty()),
        rtl: raw.is_right_to_left,
        text,
    })
}

/// Lignes du texte simple, sans les lignes vides du début et de la fin.
fn plain_lines(text: &str) -> Vec<String> {
    let lines: Vec<String> = text.lines().map(|l| l.trim().to_owned()).collect();
    let start = lines.iter().position(|l| !l.is_empty());
    let end = lines.iter().rposition(|l| !l.is_empty());
    match (start, end) {
        (Some(start), Some(end)) => lines[start..=end].to_vec(),
        _ => Vec::new(),
    }
}

/// Analyse un texte LRC : une ligne peut porter plusieurs horodatages (refrain répété),
/// les en-têtes (`[ar:…]`, `[offset:…]`) sont ignorés. Résultat trié par temps.
fn parse_lrc(text: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    for line in text.lines() {
        let mut rest = line.trim();
        let mut times = Vec::new();
        while let Some(tag_end) = rest.strip_prefix('[').and_then(|r| r.find(']')) {
            match parse_time(&rest[1..=tag_end]) {
                Some(ms) => times.push(ms),
                None => break,
            }
            rest = rest[tag_end + 2..].trim_start();
        }
        let text = rest.trim();
        lines.extend(times.into_iter().map(|time_ms| LyricLine {
            time_ms,
            text: text.to_owned(),
        }));
    }
    // Tri stable : deux lignes au même instant gardent leur ordre d'origine.
    lines.sort_by_key(|l| l.time_ms);
    lines
}

/// `mm:ss`, `mm:ss.x`, `mm:ss.xx`, `mm:ss.xxx` (ou `:` avant la fraction) → millisecondes.
fn parse_time(tag: &str) -> Option<u32> {
    let (min, rest) = tag.split_once(':')?;
    let (sec, frac) = match rest.split_once(['.', ':']) {
        Some((sec, frac)) => (sec, frac),
        None => (rest, ""),
    };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(min) || !digits(sec) || !(frac.is_empty() || digits(frac)) {
        return None;
    }
    let frac_ms = match frac.len() {
        0 => 0,
        1 => frac.parse::<u32>().ok()? * 100,
        2 => frac.parse::<u32>().ok()? * 10,
        _ => frac[..3].parse::<u32>().ok()?,
    };
    let min: u32 = min.parse().ok()?;
    let sec: u32 = sec.parse().ok()?;
    min.checked_mul(60_000)?
        .checked_add(sec.checked_mul(1000)?)?
        .checked_add(frac_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(time_ms: u32, text: &str) -> LyricLine {
        LyricLine {
            time_ms,
            text: text.to_owned(),
        }
    }

    fn raw(subtitles: Option<&str>, lyrics: Option<&str>) -> RawLyrics {
        RawLyrics {
            lyrics_provider: Some("MUSIXMATCH".into()),
            is_right_to_left: false,
            lyrics: lyrics.map(Into::into),
            subtitles: subtitles.map(Into::into),
        }
    }

    #[test]
    fn time_formats() {
        assert_eq!(parse_time("00:07.30"), Some(7_300));
        assert_eq!(parse_time("01:02.5"), Some(62_500));
        assert_eq!(parse_time("01:02.345"), Some(62_345));
        assert_eq!(parse_time("01:02.3456"), Some(62_345));
        assert_eq!(parse_time("01:02"), Some(62_000));
        assert_eq!(parse_time("01:02:50"), Some(62_500));
        assert_eq!(parse_time("100:00.00"), Some(6_000_000));
        assert_eq!(parse_time("ar:Queen"), None);
        assert_eq!(parse_time("offset:+200"), None);
        assert_eq!(parse_time(":12.00"), None);
        assert_eq!(parse_time("01:.5"), None);
    }

    #[test]
    fn lrc_lines() {
        let lrc = "[ar:Queen]\n[ti:Bohemian Rhapsody]\n\
                   [00:00.25] Is this the real life?\r\n\
                   [00:07.30]Caught in a landslide\n\
                   [00:20.00]\n\
                   texte sans horodatage\n\
                   [00:30.00][00:10.00] Refrain\n";
        assert_eq!(
            parse_lrc(lrc),
            vec![
                line(250, "Is this the real life?"),
                line(7_300, "Caught in a landslide"),
                line(10_000, "Refrain"),
                line(20_000, ""),
                line(30_000, "Refrain"),
            ]
        );
    }

    #[test]
    fn lrc_keeps_brackets_in_text() {
        assert_eq!(
            parse_lrc("[00:01.00] [Chorus] la la"),
            vec![line(1_000, "[Chorus] la la")]
        );
    }

    #[test]
    fn synced_preferred() {
        let lyrics = convert(TrackId(1), raw(Some("[00:01.00] a"), Some("a"))).unwrap();
        assert_eq!(
            lyrics.text,
            LyricsText::Synced {
                lines: vec![line(1_000, "a")]
            }
        );
        assert_eq!(lyrics.provider.as_deref(), Some("MUSIXMATCH"));
    }

    #[test]
    fn plain_fallback() {
        for subtitles in [
            None,
            Some(""),
            Some("[ar:x]\nnon horodaté"),
            Some("[00:01.00]"),
        ] {
            let lyrics = convert(TrackId(1), raw(subtitles, Some("\n a \n\n b\n\n"))).unwrap();
            assert_eq!(
                lyrics.text,
                LyricsText::Plain {
                    lines: vec!["a".into(), "".into(), "b".into()]
                }
            );
        }
    }

    #[test]
    fn nothing_usable() {
        assert_eq!(convert(TrackId(1), raw(None, None)), None);
        assert_eq!(convert(TrackId(1), raw(Some(""), Some(" \n "))), None);
    }

    #[test]
    fn serialized_shape() {
        let lyrics = convert(TrackId(7), raw(Some("[00:01.00] a"), None)).unwrap();
        let value = serde_json::to_value(&lyrics).unwrap();
        assert_eq!(value["trackId"], 7);
        assert_eq!(value["text"]["kind"], "synced");
        assert_eq!(value["text"]["lines"][0]["timeMs"], 1000);
    }
}
