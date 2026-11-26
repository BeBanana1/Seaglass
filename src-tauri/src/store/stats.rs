//! Listening statistics (#326), computed in SQL over the local history (`plays.rs`). The
//! queries cover a `Span` interval (ms since the epoch).

use rusqlite::params;

use super::{Result, Store, StoreError};
use crate::tidal::models::{ArtistSummary, Track, TrackId};

/// Interval of plays `[since, until[`, in ms since the epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub since: i64,
    pub until: i64,
}

impl Span {
    /// From `since` until now (0: the whole history).
    pub fn since(since: i64) -> Self {
        Self {
            since,
            until: i64::MAX,
        }
    }
}

/// Totals of a period.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Totals {
    pub plays: i64,
    /// Sum of the durations of the played tracks, in seconds.
    pub seconds: i64,
    /// Plays of unknown duration (recorded before 0.5.33, track not completed yet).
    pub unknown: i64,
}

/// A track of the history, its metadata if recorded, its number of plays.
pub type CountedTrack = (TrackId, Option<Track>, i64);

/// An artist in the ranking.
#[derive(Debug, Clone, PartialEq)]
pub struct TopArtist {
    pub artist: ArtistSummary,
    pub plays: i64,
    /// Photo already asked of TIDAL (`artist.picture_url` then holds what it answered).
    pub picture_known: bool,
}

/// Copy of a track kept in `played_tracks`; an unreadable one (other version) counts as
/// missing.
pub(super) fn parse_track(json: Option<String>) -> Option<Track> {
    json.and_then(|json| {
        serde_json::from_str(&json)
            .inspect_err(|e| tracing::debug!("unreadable history track: {e}"))
            .ok()
    })
}

impl Store {
    /// Most played tracks over `span`, with at least `min_plays` plays, and their metadata
    /// if recorded (`None`: a play from before 0.5.33, to be completed by
    /// `save_played_tracks`). On a tie, the most recently played first.
    pub fn top_tracks(
        &self,
        span: Span,
        min_plays: i64,
        limit: usize,
    ) -> Result<Vec<CountedTrack>> {
        self.counted_tracks(
            "SELECT p.track_id, t.json, COUNT(*) AS n, MAX(p.played_at) AS last FROM plays p
             LEFT JOIN played_tracks t ON t.id = p.track_id
             WHERE p.played_at >= ?1 AND p.played_at < ?2
             GROUP BY p.track_id HAVING n >= ?3
             ORDER BY n DESC, last DESC, MAX(p.id) DESC LIMIT ?4",
            params![span.since, span.until, min_plays, limit as i64],
        )
    }

    /// A query whose columns 0 to 2 are the track id, its JSON copy (or `NULL`) and a
    /// number of plays. An unreadable copy (another version) counts as missing.
    pub(super) fn counted_tracks(
        &self,
        sql: &str,
        args: impl rusqlite::Params,
    ) -> Result<Vec<CountedTrack>> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let mut stmt = db.conn.prepare(sql)?;
        let rows = stmt.query_map(args, |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, json, count) = row?;
            out.push((TrackId(id as u64), parse_track(json), count));
        }
        Ok(out)
    }

    /// Most played main artists, with the name from their most recent play and their photo
    /// if it was already asked for.
    pub fn top_artists(&self, span: Span, limit: usize) -> Result<Vec<TopArtist>> {
        self.ranked_artists(span, limit, false)
    }

    /// Ranking of the artists of `span`; `only_new`: only those with no play before the
    /// period (the recap's discoveries, #417).
    pub(super) fn ranked_artists(
        &self,
        span: Span,
        limit: usize,
        only_new: bool,
    ) -> Result<Vec<TopArtist>> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        // `p.artist`: a "bare" column in SQLite, the one from the row of `MAX(p.played_at)`
        // (the date, not the id: a play received from another device, #430, has a recent id).
        let mut stmt = db.conn.prepare(
            "SELECT p.artist_id, p.artist, MAX(p.played_at) AS last, COUNT(*) AS n,
                    a.id IS NOT NULL, a.picture
             FROM plays p LEFT JOIN played_artists a ON a.id = p.artist_id
             WHERE p.artist_id IS NOT NULL AND p.played_at >= ?1 AND p.played_at < ?2
               AND (?4 = 0 OR p.artist_id NOT IN (
                   SELECT artist_id FROM plays
                   WHERE artist_id IS NOT NULL AND played_at < ?1))
             GROUP BY p.artist_id ORDER BY n DESC, last DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            params![span.since, span.until, limit as i64, only_new],
            |r| {
                Ok(TopArtist {
                    artist: ArtistSummary {
                        id: r.get::<_, i64>(0)? as u64,
                        name: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                        picture_url: r.get(5)?,
                    },
                    plays: r.get(3)?,
                    picture_known: r.get(4)?,
                })
            },
        )?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Photos asked of TIDAL (`None`: artist without a photo, not to be asked again).
    pub fn save_artist_pictures(&self, pictures: &[(u64, Option<String>)]) -> Result<()> {
        let mut db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let tx = db.conn.transaction()?;
        for (id, picture) in pictures {
            tx.execute(
                "INSERT INTO played_artists (id, picture) VALUES (?1, ?2)
                 ON CONFLICT(id) DO UPDATE SET picture = excluded.picture",
                params![*id as i64, picture],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Played tracks whose metadata is missing (plays from before 0.5.33).
    pub fn unknown_tracks(&self, limit: usize) -> Result<Vec<TrackId>> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        let mut stmt = db.conn.prepare(
            "SELECT DISTINCT track_id FROM plays
             WHERE track_id NOT IN (SELECT id FROM played_tracks) LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |r| r.get::<_, i64>(0))?;
        Ok(rows
            .map(|id| id.map(|id| TrackId(id as u64)))
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn play_totals(&self, span: Span) -> Result<Totals> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        Ok(db.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(duration), 0), COUNT(*) - COUNT(duration)
             FROM plays WHERE played_at >= ?1 AND played_at < ?2",
            [span.since, span.until],
            |r| {
                Ok(Totals {
                    plays: r.get(0)?,
                    seconds: r.get(1)?,
                    unknown: r.get(2)?,
                })
            },
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::super::plays::tests::{play, track};
    use super::*;

    #[test]
    fn top_tracks_and_artists() {
        let store = Store::in_memory().unwrap();
        // (track, album, date); the artist is the album + 100.
        for (t, a, at) in [(1, 10, 1), (1, 10, 2), (2, 20, 3), (3, 20, 4), (2, 20, 50)] {
            play(&store, t, a, at);
        }
        let tracks = |since| {
            store
                .top_tracks(Span::since(since), 1, 10)
                .unwrap()
                .into_iter()
                .map(|(id, t, n)| (id.0, t.map(|t| t.title), n))
                .collect::<Vec<_>>()
        };
        // Tie between 1 and 2: track 2, played last, comes first.
        assert_eq!(
            tracks(0),
            [
                (2, Some("Titre 2".into()), 2),
                (1, Some("Titre 1".into()), 2),
                (3, Some("Titre 3".into()), 1),
            ]
        );
        assert_eq!(tracks(10), [(2, Some("Titre 2".into()), 1)]);
        let artists = || {
            store
                .top_artists(Span::since(0), 10)
                .unwrap()
                .into_iter()
                .map(|a| (a.artist.id, a.plays, a.picture_known, a.artist.picture_url))
                .collect::<Vec<_>>()
        };
        assert_eq!(artists(), [(120, 3, false, None), (110, 2, false, None)]);
        store
            .save_artist_pictures(&[(120, Some("p.jpg".into())), (110, None)])
            .unwrap();
        assert_eq!(
            artists(),
            [(120, 3, true, Some("p.jpg".into())), (110, 2, true, None)]
        );
        assert_eq!(
            store.top_artists(Span::since(0), 1).unwrap()[0].artist.name,
            "Artiste 120"
        );
        assert_eq!(
            store.play_totals(Span::since(3)).unwrap(),
            Totals {
                plays: 3,
                seconds: 600,
                unknown: 0
            }
        );
    }

    #[test]
    fn old_plays_are_completed() {
        let store = Store::in_memory().unwrap();
        {
            // A play from before 0.5.33: no recorded track and no duration.
            let db = store.db.lock().unwrap();
            db.conn
                .execute(
                    "INSERT INTO plays (track_id, album_id, played_at) VALUES (5, 10, 1)",
                    [],
                )
                .unwrap();
        }
        let top = store.top_tracks(Span::since(0), 1, 10).unwrap();
        assert_eq!(top.len(), 1);
        assert!(top[0].1.is_none());
        assert_eq!(store.play_totals(Span::since(0)).unwrap().unknown, 1);
        assert_eq!(store.unknown_tracks(10).unwrap(), [TrackId(5)]);
        store.save_played_tracks(&[track(5, 10, 110)]).unwrap();
        let top = store.top_tracks(Span::since(0), 1, 10).unwrap();
        assert_eq!(top[0].1.as_ref().map(|t| t.id), Some(TrackId(5)));
        assert!(store.unknown_tracks(10).unwrap().is_empty());
        assert_eq!(
            store.play_totals(Span::since(0)).unwrap(),
            Totals {
                plays: 1,
                seconds: 200,
                unknown: 0
            }
        );
    }
}
