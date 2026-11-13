//! Statistiques d'écoute (#326), calculées en SQL sur l'historique local (`plays.rs`).
//! Les requêtes portent sur un intervalle `Span` (ms depuis l'epoch).

use rusqlite::params;

use super::{Result, Store, StoreError};
use crate::tidal::models::{ArtistSummary, Track, TrackId};

/// Intervalle d'écoutes `[since, until[`, en ms depuis l'epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub since: i64,
    pub until: i64,
}

impl Span {
    /// Depuis `since` jusqu'à maintenant (0 : tout l'historique).
    pub fn since(since: i64) -> Self {
        Self {
            since,
            until: i64::MAX,
        }
    }
}

/// Totaux d'une période.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Totals {
    pub plays: i64,
    /// Somme des durées des titres écoutés, en secondes.
    pub seconds: i64,
    /// Écoutes de durée inconnue (enregistrées avant la 0.5.33, titre pas encore complété).
    pub unknown: i64,
}

/// Titre de l'historique, ses métadonnées si elles sont enregistrées, son nombre d'écoutes.
pub type CountedTrack = (TrackId, Option<Track>, i64);

/// Artiste du classement.
#[derive(Debug, Clone, PartialEq)]
pub struct TopArtist {
    pub artist: ArtistSummary,
    pub plays: i64,
    /// Photo déjà demandée à TIDAL (`artist.picture_url` vaut alors ce qu'il a répondu).
    pub picture_known: bool,
}

impl Store {
    /// Titres les plus écoutés sur `span`, avec au moins `min_plays` écoutes, et leurs
    /// métadonnées si elles sont enregistrées (`None` : écoute d'avant la 0.5.33, à
    /// compléter par `save_played_tracks`). À égalité, le plus récemment écouté d'abord.
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

    /// Requête dont les colonnes 0 à 2 sont l'id du titre, sa copie JSON (ou `NULL`) et un
    /// nombre d'écoutes. Une copie illisible (autre version) compte comme absente.
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
            let track = json.and_then(|json| {
                serde_json::from_str(&json)
                    .inspect_err(|e| tracing::debug!("titre de l'historique illisible : {e}"))
                    .ok()
            });
            out.push((TrackId(id as u64), track, count));
        }
        Ok(out)
    }

    /// Artistes principaux les plus écoutés, avec le nom de leur écoute la plus récente et
    /// leur photo si elle a déjà été demandée.
    pub fn top_artists(&self, span: Span, limit: usize) -> Result<Vec<TopArtist>> {
        self.ranked_artists(span, limit, false)
    }

    /// Classement des artistes de `span` ; `only_new` : seulement ceux qui n'ont aucune
    /// écoute avant la période (découvertes de la rétrospective, #417).
    pub(super) fn ranked_artists(
        &self,
        span: Span,
        limit: usize,
        only_new: bool,
    ) -> Result<Vec<TopArtist>> {
        let db = self.db.lock().map_err(|_| StoreError::Poisoned)?;
        // `p.artist` : colonne « nue » de SQLite, celle de la ligne de `MAX(p.played_at)`
        // (la date, pas l'id : une écoute reçue d'un autre appareil, #430, a un id récent).
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

    /// Photos demandées à TIDAL (`None` : artiste sans photo, à ne pas redemander).
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

    /// Titres écoutés dont les métadonnées manquent (écoutes d'avant la 0.5.33).
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
        // (titre, album, date) ; l'artiste est l'album + 100.
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
        // Égalité 1 / 2 : le titre 2, écouté en dernier, passe devant.
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
            // Écoute d'avant la 0.5.33 : ni titre enregistré ni durée.
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
