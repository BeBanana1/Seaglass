//! Format actually played with each listen (#524): quality served by TIDAL, codec, bit depth,
//! sample rate and whether the signal reached the device bit-perfect. Recorded from 0.5.103
//! on (older listens have `NULL`), carried by the history sync (#430), summed up for the
//! recap's audio panel.

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use super::stats::Span;
use super::{Result, Store, StoreError};

/// Formats listed in the recap.
const TOP_FORMATS: usize = 5;
/// Qualities TIDAL serves (`AudioQuality`, as stored).
const QUALITIES: [&str; 4] = ["LOW", "HIGH", "LOSSLESS", "HI_RES_LOSSLESS"];
const MAX_CODEC_CHARS: usize = 16;

/// Format of a listen. `quality` is a plain string rather than `AudioQuality`: a value
/// unknown to this version, received from a newer device, must not reject the whole batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayFormat {
    pub quality: String,
    pub codec: String,
    #[serde(default)]
    pub bit_depth: Option<u8>,
    #[serde(default)]
    pub sample_rate: Option<u32>,
    #[serde(default)]
    pub bit_perfect: bool,
}

impl PlayFormat {
    /// Received from another device: `None` if the quality is unknown or the codec odd.
    pub fn sanitize(self) -> Option<Self> {
        let codec_ok = !self.codec.is_empty()
            && self.codec.len() <= MAX_CODEC_CHARS
            && self
                .codec
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.');
        let rate_ok = self.sample_rate.is_none_or(|r| r <= 1_000_000);
        (QUALITIES.contains(&self.quality.as_str()) && codec_ok && rate_ok).then_some(self)
    }
}

/// One of the formats played most.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountedFormat {
    pub codec: String,
    pub bit_depth: Option<u8>,
    pub sample_rate: Option<u32>,
    pub plays: i64,
}

/// Audio side of a period, over the listens that have a format.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AudioTotals {
    /// Listens with a known format; the others are not counted below.
    pub known: i64,
    pub hires: i64,
    pub lossless: i64,
    pub lossy: i64,
    pub bit_perfect: i64,
    /// Sum of the durations of the bit-perfect listens, in seconds.
    pub bit_perfect_seconds: i64,
    pub formats: Vec<CountedFormat>,
}

/// Adds the format columns (#524), all nullable.
pub(super) fn migrate(conn: &Connection) -> Result<()> {
    let migrated: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('plays') WHERE name = 'quality'",
        [],
        |r| r.get(0),
    )?;
    if !migrated {
        conn.execute_batch(
            "BEGIN;
             ALTER TABLE plays ADD COLUMN quality TEXT;
             ALTER TABLE plays ADD COLUMN codec TEXT;
             ALTER TABLE plays ADD COLUMN bit_depth INTEGER;
             ALTER TABLE plays ADD COLUMN sample_rate INTEGER;
             ALTER TABLE plays ADD COLUMN bit_perfect INTEGER;
             COMMIT;",
        )?;
    }
    Ok(())
}

/// Columns of `plays` written from a `PlayFormat`, in this order (`?n` to `?n+4`).
pub(super) const COLUMNS: &str = "quality, codec, bit_depth, sample_rate, bit_perfect";

/// Values of `COLUMNS`: quality, codec, bit depth, sample rate, bit-perfect.
pub(super) type Values<'a> = (
    Option<&'a str>,
    Option<&'a str>,
    Option<u8>,
    Option<u32>,
    Option<bool>,
);

/// Values for `COLUMNS`.
pub(super) fn values(format: Option<&PlayFormat>) -> Values<'_> {
    match format {
        Some(f) => (
            Some(&f.quality),
            Some(&f.codec),
            f.bit_depth,
            f.sample_rate,
            Some(f.bit_perfect),
        ),
        None => (None, None, None, None, None),
    }
}

/// Reads `COLUMNS` starting at column `at`.
pub(super) fn read(r: &rusqlite::Row, at: usize) -> rusqlite::Result<Option<PlayFormat>> {
    let Some(quality) = r.get::<_, Option<String>>(at)? else {
        return Ok(None);
    };
    Ok(Some(PlayFormat {
        quality,
        codec: r.get::<_, Option<String>>(at + 1)?.unwrap_or_default(),
        bit_depth: r.get(at + 2)?,
        sample_rate: r.get(at + 3)?,
        bit_perfect: r.get::<_, Option<bool>>(at + 4)?.unwrap_or(false),
    }))
}

impl Store {
    pub fn audio_totals(&self, span: Span) -> Result<AudioTotals> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let mut totals = db.conn.query_row(
            "SELECT COUNT(quality),
                    COALESCE(SUM(quality = 'HI_RES_LOSSLESS'), 0),
                    COALESCE(SUM(quality = 'LOSSLESS'), 0),
                    COALESCE(SUM(quality IN ('LOW', 'HIGH')), 0),
                    COALESCE(SUM(bit_perfect = 1), 0),
                    COALESCE(SUM(CASE WHEN bit_perfect = 1 THEN duration END), 0)
             FROM plays WHERE played_at >= ?1 AND played_at < ?2",
            [span.since, span.until],
            |r| {
                Ok(AudioTotals {
                    known: r.get(0)?,
                    hires: r.get(1)?,
                    lossless: r.get(2)?,
                    lossy: r.get(3)?,
                    bit_perfect: r.get(4)?,
                    bit_perfect_seconds: r.get(5)?,
                    formats: Vec::new(),
                })
            },
        )?;
        let mut stmt = db.conn.prepare(
            "SELECT codec, bit_depth, sample_rate, COUNT(*) AS n FROM plays
             WHERE quality IS NOT NULL AND played_at >= ?1 AND played_at < ?2
             GROUP BY codec, bit_depth, sample_rate
             ORDER BY n DESC, sample_rate DESC, bit_depth DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![span.since, span.until, TOP_FORMATS as i64], |r| {
            Ok(CountedFormat {
                codec: r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                bit_depth: r.get(1)?,
                sample_rate: r.get(2)?,
                plays: r.get(3)?,
            })
        })?;
        totals.formats = rows.collect::<rusqlite::Result<_>>()?;
        Ok(totals)
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::plays::Play;
    use super::super::plays::tests::{album, track};
    use super::*;

    pub fn format(quality: &str, depth: u8, rate: u32, bit_perfect: bool) -> PlayFormat {
        PlayFormat {
            quality: quality.into(),
            codec: "flac".into(),
            bit_depth: Some(depth),
            sample_rate: Some(rate),
            bit_perfect,
        }
    }

    fn play(store: &Store, id: u64, at: i64, format: Option<PlayFormat>) {
        store
            .add_play(&Play {
                track: &track(id, 10, 110),
                album: &album(10),
                source: None,
                played_at: at,
                format: format.as_ref(),
            })
            .unwrap();
    }

    #[test]
    fn totals_count_only_known_formats() {
        let store = Store::in_memory().unwrap();
        play(&store, 1, 1, None);
        play(
            &store,
            2,
            2,
            Some(format("HI_RES_LOSSLESS", 24, 96000, true)),
        );
        play(
            &store,
            3,
            3,
            Some(format("HI_RES_LOSSLESS", 24, 96000, false)),
        );
        play(&store, 4, 4, Some(format("LOSSLESS", 16, 44100, true)));
        let lossy = PlayFormat {
            codec: "mp4a.40.2".into(),
            bit_depth: None,
            ..format("HIGH", 0, 44100, false)
        };
        play(&store, 5, 5, Some(lossy));

        let totals = store.audio_totals(Span::since(0)).unwrap();
        assert_eq!(
            (totals.known, totals.hires, totals.lossless, totals.lossy),
            (4, 2, 1, 1)
        );
        // Durations of the test tracks: 200 s.
        assert_eq!((totals.bit_perfect, totals.bit_perfect_seconds), (2, 400));
        let formats: Vec<_> = totals
            .formats
            .iter()
            .map(|f| (f.codec.as_str(), f.bit_depth, f.sample_rate, f.plays))
            .collect();
        assert_eq!(
            formats,
            [
                ("flac", Some(24), Some(96000), 2),
                ("flac", Some(16), Some(44100), 1),
                ("mp4a.40.2", None, Some(44100), 1),
            ]
        );
        assert_eq!(
            store.audio_totals(Span::since(10)).unwrap(),
            AudioTotals::default()
        );
    }

    #[test]
    fn received_formats_are_checked() {
        let good = format("LOSSLESS", 16, 44100, false);
        assert_eq!(good.clone().sanitize(), Some(good.clone()));
        let unknown = PlayFormat {
            quality: "DOLBY_ATMOS".into(),
            ..good.clone()
        };
        assert_eq!(unknown.sanitize(), None);
        let odd = PlayFormat {
            codec: "flac; DROP TABLE".into(),
            ..good
        };
        assert_eq!(odd.sanitize(), None);
    }
}
