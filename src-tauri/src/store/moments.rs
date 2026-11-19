//! Recap timeline and highlights (#521): plays per month or per day, longest streak of
//! listening days, most replayed track in a single day, first track of the period. Local
//! time, like the rest of the recap (`recap.rs`).

use rusqlite::params;

use super::recap::{CivilPeriod, LOCAL};
use super::stats::{CountedTrack, Span, parse_track};
use super::{Result, Store, StoreError};
use crate::tidal::models::TrackId;

/// Track played the most times in a single day of the period.
#[derive(Debug, Clone, PartialEq)]
pub struct TopDay {
    pub track: CountedTrack,
    /// Local day, `YYYY-MM-DD`.
    pub date: String,
}

/// Highlights of a period.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Moments {
    /// Longest run of consecutive local days with at least one play.
    pub streak: i64,
    /// Absent when no track was played twice on the same day.
    pub top_day: Option<TopDay>,
    /// First play of the period (play count: 1).
    pub first: Option<CountedTrack>,
}

impl Store {
    /// Plays per month (12 slots) for a year, per day for a month.
    pub fn timeline(&self, period: CivilPeriod) -> Result<Vec<i64>> {
        let span = self.civil_span(period)?;
        let (format, slots) = match period.month {
            Some(month) => ("%d", days_in_month(period.year, month)),
            None => ("%m", 12),
        };
        let mut out = vec![0; slots];
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let mut stmt = db.conn.prepare(&format!(
            "SELECT CAST(strftime('{format}', {LOCAL}) AS INTEGER) AS k, COUNT(*)
             FROM plays WHERE played_at >= ?1 AND played_at < ?2 GROUP BY k"
        ))?;
        let rows = stmt.query_map([span.since, span.until], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (key, count) = row?;
            // Months and days start at 1.
            if let Some(slot) = usize::try_from(key - 1).ok().and_then(|k| out.get_mut(k)) {
                *slot = count;
            }
        }
        Ok(out)
    }

    pub fn moments(&self, span: Span) -> Result<Moments> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        // Julian day number of the local day: consecutive days differ by 1.
        let mut stmt = db.conn.prepare(&format!(
            "SELECT DISTINCT CAST(julianday({LOCAL}, 'start of day') AS INTEGER) AS d
             FROM plays WHERE played_at >= ?1 AND played_at < ?2 ORDER BY d"
        ))?;
        let days = stmt
            .query_map([span.since, span.until], |r| r.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        // `played_at` alone is unambiguous: `played_tracks` has no such column.
        let mut stmt = db.conn.prepare(&format!(
            "SELECT p.track_id, t.json, COUNT(*) AS n, strftime('%Y-%m-%d', {LOCAL}) AS d,
                    MAX(p.played_at) AS last
             FROM plays p LEFT JOIN played_tracks t ON t.id = p.track_id
             WHERE p.played_at >= ?1 AND p.played_at < ?2
             GROUP BY d, p.track_id HAVING n >= 2
             ORDER BY n DESC, last DESC LIMIT 1"
        ))?;
        let top_day = stmt
            .query_map([span.since, span.until], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?
            .next()
            .transpose()?
            .map(|(id, json, plays, date)| TopDay {
                track: (TrackId(id as u64), parse_track(json), plays),
                date,
            });

        let mut stmt = db.conn.prepare(
            "SELECT p.track_id, t.json FROM plays p
             LEFT JOIN played_tracks t ON t.id = p.track_id
             WHERE p.played_at >= ?1 AND p.played_at < ?2
             ORDER BY p.played_at, p.id LIMIT 1",
        )?;
        let first = stmt
            .query_map(params![span.since, span.until], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?))
            })?
            .next()
            .transpose()?
            .map(|(id, json)| (TrackId(id as u64), parse_track(json), 1));

        Ok(Moments {
            streak: longest_run(&days),
            top_day,
            first,
        })
    }
}

/// Longest run of consecutive numbers in a sorted list without duplicates.
fn longest_run(days: &[i64]) -> i64 {
    let mut best = 0;
    let mut run = 0;
    let mut previous: Option<i64> = None;
    for &day in days {
        run = if previous == Some(day - 1) {
            run + 1
        } else {
            1
        };
        best = best.max(run);
        previous = Some(day);
    }
    best
}

fn days_in_month(year: i32, month: u32) -> usize {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[cfg(test)]
mod tests {
    use super::super::plays::tests::play;
    use super::*;

    const HOUR: i64 = 3600 * 1000;
    const DAY: i64 = 24 * HOUR;

    #[test]
    fn runs_and_month_lengths() {
        assert_eq!(longest_run(&[]), 0);
        assert_eq!(longest_run(&[5]), 1);
        assert_eq!(longest_run(&[1, 2, 4, 5, 6, 9]), 3);
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2100, 2), 28);
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(2026, 4), 30);
        assert_eq!(days_in_month(2026, 12), 31);
    }

    #[test]
    fn timeline_and_moments() {
        let store = Store::in_memory().unwrap();
        let period = |month| CivilPeriod { year: 2026, month };
        let june = store.civil_span(period(Some(6))).unwrap();
        // June: track 1 on the 1st, track 2 three times on the 2nd, track 3 on the 3rd and
        // the 10th. Noon, away from midnight and daylight saving changes.
        let noon = june.since + 12 * HOUR;
        play(&store, 1, 10, noon);
        for i in 0..3 {
            play(&store, 2, 20, noon + DAY + i);
        }
        play(&store, 3, 30, noon + 2 * DAY);
        play(&store, 3, 30, noon + 9 * DAY);

        let days = store.timeline(period(Some(6))).unwrap();
        assert_eq!(days.len(), 30);
        assert_eq!(&days[..4], [1, 3, 1, 0]);
        assert_eq!(days[9], 1);
        let months = store.timeline(period(None)).unwrap();
        assert_eq!(months.len(), 12);
        assert_eq!(months[5], 6);
        assert_eq!(months.iter().sum::<i64>(), 6);

        let moments = store.moments(june).unwrap();
        assert_eq!(moments.streak, 3);
        let top = moments.top_day.unwrap();
        assert_eq!((top.track.0, top.track.2), (TrackId(2), 3));
        assert!(top.track.1.is_some());
        assert!(top.date.ends_with("-06-02"), "{}", top.date);
        assert_eq!(moments.first.map(|f| f.0), Some(TrackId(1)));

        // No track played twice on one day: no top day.
        let july = store.civil_span(period(Some(7))).unwrap();
        play(&store, 4, 40, july.since + 12 * HOUR);
        let moments = store.moments(july).unwrap();
        assert_eq!(moments.streak, 1);
        assert_eq!(moments.top_day, None);
        let august = store.civil_span(period(Some(8))).unwrap();
        assert_eq!(store.moments(august).unwrap(), Moments::default());
    }
}
