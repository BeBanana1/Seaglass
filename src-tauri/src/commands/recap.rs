//! Rétrospective du mois et de l'année (#417) : classements, temps d'écoute, découvertes et
//! habitudes d'un mois ou d'une année civils, en heure locale. Tout vient de l'historique
//! de cet appareil (`store::recap`, `store::stats`) ; seuls les titres d'anciennes écoutes
//! et les photos des artistes sont demandés à TIDAL, comme pour les statistiques.

use serde::Serialize;
use tauri::State;

use super::play_stats::{Counted, blocking, complete, pictured, start_backfill};
use super::{AppState, CmdResult};
use crate::store::{CivilPeriod, Habits};
use crate::tidal::models::{AlbumSummary, ArtistSummary, Track};

const LIMIT: usize = 10;

/// Période qui a des écoutes. `month` absent : l'année entière.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecapPeriod {
    pub year: i32,
    pub month: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recap {
    pub plays: i64,
    /// Temps d'écoute approché, en secondes (voir `PlayStats`).
    pub seconds: i64,
    pub unknown: i64,
    /// Jours avec au moins une écoute.
    pub days: i64,
    pub tracks: Vec<Counted<Track>>,
    pub albums: Vec<Counted<AlbumSummary>>,
    pub artists: Vec<Counted<ArtistSummary>>,
    /// Artistes sans aucune écoute avant la période, les plus écoutés d'abord.
    pub new_artists: Vec<Counted<ArtistSummary>>,
    pub new_artist_count: i64,
    pub new_track_count: i64,
    /// Écoutes par jour de la semaine, lundi en premier.
    pub weekdays: Vec<i64>,
    /// Écoutes par heure locale, de 0 à 23.
    pub hours: Vec<i64>,
    /// Jour le plus écouté (0 = lundi), absent sans écoute.
    pub top_weekday: Option<usize>,
    pub top_hour: Option<usize>,
}

/// Mois qui ont des écoutes, groupés sous leur année : l'année d'abord, puis ses mois, du
/// plus récent au plus ancien.
#[tauri::command]
pub async fn recap_periods(state: State<'_, AppState>) -> CmdResult<Vec<RecapPeriod>> {
    let Some(store) = state.store.clone() else {
        return Ok(Vec::new());
    };
    let months = blocking(&store, |s| s.recap_months()).await?;
    Ok(with_years(&months))
}

#[tauri::command]
pub async fn recap(state: State<'_, AppState>, year: i32, month: Option<u32>) -> CmdResult<Recap> {
    if !(1970..=9999).contains(&year) || month.is_some_and(|m| !(1..=12).contains(&m)) {
        return Err("période invalide".into());
    }
    let Some(store) = state.store.clone() else {
        return Ok(Recap::default());
    };
    let period = CivilPeriod { year, month };
    let span = blocking(&store, move |s| s.civil_span(period)).await?;
    let top = blocking(&store, move |s| s.top_tracks(span, 1, LIMIT)).await?;
    let tracks = complete(&state.tidal, &store, top).await;
    let (top_artists, new_artists) = blocking(&store, move |s| {
        Ok((s.top_artists(span, LIMIT)?, s.new_artists(span, LIMIT)?))
    })
    .await?;
    let artists = pictured(&state.tidal, &store, top_artists).await;
    let new_artists = pictured(&state.tidal, &store, new_artists).await;
    let recap = blocking(&store, move |s| {
        let totals = s.play_totals(span)?;
        let (new_artist_count, new_track_count) = s.discovery_counts(span)?;
        let habits = s.habits(span)?;
        let albums = s.top_albums(span, 1, LIMIT)?;
        Ok(Recap {
            plays: totals.plays,
            seconds: totals.seconds,
            unknown: totals.unknown,
            days: habits.days,
            tracks,
            albums: albums
                .into_iter()
                .map(|(item, plays)| Counted { item, plays })
                .collect(),
            artists,
            new_artists,
            new_artist_count,
            new_track_count,
            ..from_habits(&habits)
        })
    })
    .await?;
    if recap.unknown > 0 {
        start_backfill(&state.tidal, &store);
    }
    Ok(recap)
}

/// Ajoute chaque année devant ses mois (déjà triés du plus récent au plus ancien).
fn with_years(months: &[CivilPeriod]) -> Vec<RecapPeriod> {
    let mut out: Vec<RecapPeriod> = Vec::new();
    for m in months {
        if out.last().is_none_or(|p| p.year != m.year) {
            out.push(RecapPeriod {
                year: m.year,
                month: None,
            });
        }
        out.push(RecapPeriod {
            year: m.year,
            month: m.month,
        });
    }
    out
}

/// Répartitions et maxima ; la semaine commence le lundi. À égalité, le premier l'emporte.
fn from_habits(habits: &Habits) -> Recap {
    let weekdays: Vec<i64> = (1..=7).map(|d| habits.weekdays[d % 7]).collect();
    let hours = habits.hours.to_vec();
    Recap {
        top_weekday: busiest(&weekdays),
        top_hour: busiest(&hours),
        weekdays,
        hours,
        ..Recap::default()
    }
}

fn busiest(counts: &[i64]) -> Option<usize> {
    let max = *counts.iter().max()?;
    if max == 0 {
        return None;
    }
    counts.iter().position(|&n| n == max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn years_come_before_their_months() {
        let months = [(2026, 3), (2026, 1), (2025, 12)].map(|(year, m)| CivilPeriod {
            year,
            month: Some(m),
        });
        let out: Vec<_> = with_years(&months)
            .into_iter()
            .map(|p| (p.year, p.month))
            .collect();
        assert_eq!(
            out,
            [
                (2026, None),
                (2026, Some(3)),
                (2026, Some(1)),
                (2025, None),
                (2025, Some(12))
            ]
        );
    }

    #[test]
    fn habits_start_on_monday() {
        let mut habits = Habits::default();
        habits.weekdays[0] = 5; // dimanche
        habits.weekdays[1] = 2; // lundi
        habits.hours[22] = 4;
        habits.hours[8] = 4;
        let recap = from_habits(&habits);
        assert_eq!(recap.weekdays, [2, 0, 0, 0, 0, 0, 5]);
        assert_eq!(recap.top_weekday, Some(6));
        // Égalité : la première heure l'emporte.
        assert_eq!(recap.top_hour, Some(8));
        assert_eq!(from_habits(&Habits::default()).top_hour, None);
    }
}
