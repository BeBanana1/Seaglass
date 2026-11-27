//! Recap of the month and the year (#417): calendar periods in local time and listening
//! habits, computed in SQL over the history (`plays.rs`). The rankings come from
//! `stats.rs`, over the period's interval.
//!
//! Local time is SQLite's (`localtime` / `utc` modifiers, the system's time zone, clock
//! changes included): nothing to compute again on the Rust side.

use rusqlite::params;

use super::stats::{Span, TopArtist};
use super::{Result, Store, StoreError};

/// Calendar month (`month` from 1 to 12) or a whole year (no `month`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CivilPeriod {
    pub year: i32,
    pub month: Option<u32>,
}

/// Listening habits of a period, in number of plays.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Habits {
    /// (Local) days with at least one play.
    pub days: i64,
    /// Per weekday, Sunday first (SQLite `%w`).
    pub weekdays: [i64; 7],
    /// Per local hour, from 0 to 23.
    pub hours: [i64; 24],
    /// By weekday (Sunday first) then local hour: the heatmap (#521).
    pub grid: [[i64; 24]; 7],
}

/// SQL arguments for the local time of a play (`played_at` in ms).
pub(super) const LOCAL: &str = "played_at / 1000, 'unixepoch', 'localtime'";

impl Store {
    /// The period's interval in local time: from midnight on the first day to midnight on
    /// the first day of the next period.
    pub fn civil_span(&self, period: CivilPeriod) -> Result<Span> {
        let (start, step) = match period.month {
            Some(month) => (format!("{:04}-{month:02}-01", period.year), "+1 month"),
            None => (format!("{:04}-01-01", period.year), "+1 year"),
        };
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        Ok(db.conn.query_row(
            "SELECT CAST(strftime('%s', ?1, 'utc') AS INTEGER) * 1000,
                    CAST(strftime('%s', ?1, ?2, 'utc') AS INTEGER) * 1000",
            params![start, step],
            |r| {
                Ok(Span {
                    since: r.get(0)?,
                    until: r.get(1)?,
                })
            },
        )?)
    }

    /// (Local) months that have plays, most recent first.
    pub fn recap_months(&self) -> Result<Vec<CivilPeriod>> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let mut stmt = db.conn.prepare(&format!(
            "SELECT DISTINCT CAST(strftime('%Y', {LOCAL}) AS INTEGER) AS y,
                             CAST(strftime('%m', {LOCAL}) AS INTEGER) AS m
             FROM plays ORDER BY y DESC, m DESC"
        ))?;
        let rows = stmt.query_map([], |r| {
            Ok(CivilPeriod {
                year: r.get(0)?,
                month: Some(r.get(1)?),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn habits(&self, span: Span) -> Result<Habits> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let mut habits = Habits {
            days: db.conn.query_row(
                &format!(
                    "SELECT COUNT(DISTINCT strftime('%Y-%m-%d', {LOCAL}))
                     FROM plays WHERE played_at >= ?1 AND played_at < ?2"
                ),
                [span.since, span.until],
                |r| r.get(0),
            )?,
            ..Habits::default()
        };
        // One pass over (weekday, hour): the two distributions are sums of the grid.
        let mut stmt = db.conn.prepare(&format!(
            "SELECT CAST(strftime('%w', {LOCAL}) AS INTEGER) AS w,
                    CAST(strftime('%H', {LOCAL}) AS INTEGER) AS h, COUNT(*)
             FROM plays WHERE played_at >= ?1 AND played_at < ?2 GROUP BY w, h"
        ))?;
        let rows = stmt.query_map([span.since, span.until], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        for row in rows {
            let (day, hour, count) = row?;
            let (Ok(day), Ok(hour)) = (usize::try_from(day), usize::try_from(hour)) else {
                continue;
            };
            if day < 7 && hour < 24 {
                habits.grid[day][hour] = count;
                habits.weekdays[day] += count;
                habits.hours[hour] += count;
            }
        }
        Ok(habits)
    }

    /// Artists discovered over `span` (no play before), most played first.
    pub fn new_artists(&self, span: Span, limit: usize) -> Result<Vec<TopArtist>> {
        self.ranked_artists(span, limit, true)
    }

    /// Numbers of artists and tracks discovered over `span` (no play before).
    pub fn discovery_counts(&self, span: Span) -> Result<(i64, i64)> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let count = |column: &str| -> Result<i64> {
            Ok(db.conn.query_row(
                &format!(
                    "SELECT COUNT(DISTINCT {column}) FROM plays
                     WHERE {column} IS NOT NULL AND played_at >= ?1 AND played_at < ?2
                       AND {column} NOT IN (SELECT {column} FROM plays
                           WHERE {column} IS NOT NULL AND played_at < ?1)"
                ),
                [span.since, span.until],
                |r| r.get(0),
            )?)
        };
        Ok((count("artist_id")?, count("track_id")?))
    }
}

#[cfg(test)]
mod tests {
    use super::super::plays::tests::play;
    use super::*;

    const HOUR: i64 = 3600 * 1000;

    #[test]
    fn civil_spans_follow_the_calendar() {
        let store = Store::in_memory().unwrap();
        let span = |year, month| store.civil_span(CivilPeriod { year, month }).unwrap();
        let march = span(2026, Some(3));
        let april = span(2026, Some(4));
        assert_eq!(march.until, april.since);
        // 31 days, give or take an hour (clock change in March, depending on the time zone).
        assert!((march.until - march.since - 31 * 24 * HOUR).abs() <= HOUR);
        let year = span(2026, None);
        assert_eq!(year.since, span(2026, Some(1)).since);
        assert_eq!(year.until, span(2027, Some(1)).since);
        assert_eq!(span(2026, Some(12)).until, year.until);
    }

    #[test]
    fn months_habits_and_discoveries() {
        let store = Store::in_memory().unwrap();
        let span = |month| {
            store
                .civil_span(CivilPeriod {
                    year: 2026,
                    month: Some(month),
                })
                .unwrap()
        };
        let feb = span(2);
        let march = span(3);
        // February: track 1 (artist 110). March (the 1st is a Sunday): track 1 at
        // midnight, tracks 2 and 3 (artist 120) at 1 am, track 3 the next day at 1 am.
        play(&store, 1, 10, feb.since + HOUR);
        play(&store, 1, 10, march.since);
        play(&store, 2, 20, march.since + HOUR);
        play(&store, 3, 20, march.since + HOUR + 1);
        play(&store, 3, 20, march.since + 25 * HOUR);
        let months: Vec<_> = store
            .recap_months()
            .unwrap()
            .into_iter()
            .map(|p| p.month)
            .collect();
        assert_eq!(months, [Some(3), Some(2)]);

        let habits = store.habits(march).unwrap();
        assert_eq!(habits.days, 2);
        assert_eq!(habits.weekdays[0], 3);
        assert_eq!(habits.weekdays[1], 1);
        assert_eq!(habits.hours[0], 1);
        assert_eq!(habits.hours[1], 3);
        assert_eq!(habits.hours.iter().sum::<i64>(), 4);
        assert_eq!(habits.grid[0][1], 2);
        assert_eq!(habits.grid[1][1], 1);
        assert_eq!(habits.grid[0][0], 1);

        // March's discoveries: artist 120 and tracks 2 and 3, not track 1.
        assert_eq!(store.discovery_counts(march).unwrap(), (1, 2));
        let new: Vec<_> = store
            .new_artists(march, 10)
            .unwrap()
            .into_iter()
            .map(|a| (a.artist.id, a.plays))
            .collect();
        assert_eq!(new, [(120, 3)]);
        assert_eq!(store.discovery_counts(feb).unwrap(), (1, 1));
        assert_eq!(store.habits(span(4)).unwrap(), Habits::default());
    }
}
