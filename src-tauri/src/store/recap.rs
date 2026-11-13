//! Rétrospective du mois et de l'année (#417) : périodes civiles en heure locale et
//! habitudes d'écoute, calculées en SQL sur l'historique (`plays.rs`). Les classements
//! viennent de `stats.rs`, sur l'intervalle de la période.
//!
//! L'heure locale est celle de SQLite (modificateurs `localtime` / `utc`, fuseau du
//! système, changements d'heure compris) : rien à recalculer côté Rust.

use rusqlite::params;

use super::stats::{Span, TopArtist};
use super::{Result, Store, StoreError};

/// Mois civil (`month` de 1 à 12) ou année entière (`month` absent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CivilPeriod {
    pub year: i32,
    pub month: Option<u32>,
}

/// Habitudes d'écoute d'une période, en nombre d'écoutes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Habits {
    /// Jours (locaux) avec au moins une écoute.
    pub days: i64,
    /// Par jour de la semaine, dimanche en premier (`%w` de SQLite).
    pub weekdays: [i64; 7],
    /// Par heure locale, de 0 à 23.
    pub hours: [i64; 24],
}

/// Arguments SQL de l'heure locale d'une écoute (`played_at` en ms).
const LOCAL: &str = "played_at / 1000, 'unixepoch', 'localtime'";

impl Store {
    /// Intervalle de la période en heure locale : de minuit le premier jour à minuit le
    /// premier jour de la période suivante.
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

    /// Mois (locaux) qui ont des écoutes, du plus récent au plus ancien.
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
        for (format, slots) in [
            ("%w", &mut habits.weekdays[..]),
            ("%H", &mut habits.hours[..]),
        ] {
            let mut stmt = db.conn.prepare(&format!(
                "SELECT CAST(strftime('{format}', {LOCAL}) AS INTEGER) AS k, COUNT(*)
                 FROM plays WHERE played_at >= ?1 AND played_at < ?2 GROUP BY k"
            ))?;
            let rows = stmt.query_map([span.since, span.until], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
            })?;
            for row in rows {
                let (key, count) = row?;
                if let Some(slot) = usize::try_from(key).ok().and_then(|k| slots.get_mut(k)) {
                    *slot = count;
                }
            }
        }
        Ok(habits)
    }

    /// Artistes découverts sur `span` (aucune écoute avant), les plus écoutés d'abord.
    pub fn new_artists(&self, span: Span, limit: usize) -> Result<Vec<TopArtist>> {
        self.ranked_artists(span, limit, true)
    }

    /// Nombres d'artistes et de titres découverts sur `span` (aucune écoute avant).
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
        // 31 jours, à une heure près (changement d'heure en mars selon le fuseau).
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
        // Février : titre 1 (artiste 110). Mars (le 1er est un dimanche) : titre 1 à
        // minuit, titres 2 et 3 (artiste 120) à 1 h, titre 3 le lendemain à 1 h.
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

        // Découvertes de mars : l'artiste 120 et les titres 2 et 3, pas le titre 1.
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
