//! Titres et artistes bloqués (#334), enregistrés chez TIDAL : ils valent sur tous les
//! appareils, et les radios du service en tiennent déjà compte.
//!
//! Endpoints vérifiés le 2026-11-12 avec le client PKCE : `users/{id}/blocks/tracks` et
//! `…/artists` (paginés, `{ created, item, type }` comme les favoris), ajout par `POST`
//! (formulaire `trackId` / `artistId`, 200), retrait par `DELETE …/{id}` (204), tous deux
//! idempotents.
//!
//! Les identifiants sont gardés en mémoire pour filtrer les mixes et les radios (`mix.rs`) :
//! un blocage fait à l'instant, ou un mix pas encore régénéré, passeraient sinon. Rien
//! d'autre n'est filtré : un titre bloqué reste lisible depuis un album ou une playlist.

use std::collections::HashSet;
use std::sync::Mutex;

use reqwest::Method;

use super::Result;
use super::api::{API_V1, TidalClient};
use super::catalog::PAGE_SIZE;
use super::models::{ArtistSummary, BlockTarget, Blocked, Track, TrackId};
use super::raw::{RawArtistFull, RawFavorite, RawTrack};

/// Plafond de chaque liste (une poignée d'éléments en pratique).
const BLOCKS_MAX: usize = 5_000;

#[derive(Default)]
struct Sets {
    tracks: HashSet<TrackId>,
    artists: HashSet<u64>,
}

impl Sets {
    fn allows(&self, track: &Track) -> bool {
        !self.tracks.contains(&track.id)
            && !track.artists.iter().any(|a| self.artists.contains(&a.id))
    }
}

/// Identifiants bloqués ; `None` tant que les listes n'ont pas été lues.
#[derive(Default)]
pub struct Blocks {
    sets: Mutex<Option<Sets>>,
}

impl Blocks {
    fn loaded(&self) -> bool {
        self.sets.lock().is_ok_and(|s| s.is_some())
    }

    fn load(&self, blocked: &Blocked) {
        if let Ok(mut guard) = self.sets.lock() {
            *guard = Some(Sets {
                tracks: blocked.tracks.iter().map(|t| t.id).collect(),
                artists: blocked.artists.iter().map(|a| a.id).collect(),
            });
        }
    }

    /// Applique un blocage ou un déblocage, si les listes sont chargées.
    fn set(&self, target: &BlockTarget, blocked: bool) {
        let Ok(mut guard) = self.sets.lock() else {
            return;
        };
        let Some(sets) = guard.as_mut() else {
            return;
        };
        match (target, blocked) {
            (BlockTarget::Track { id }, true) => sets.tracks.insert(*id),
            (BlockTarget::Track { id }, false) => sets.tracks.remove(id),
            (BlockTarget::Artist { id }, true) => sets.artists.insert(*id),
            (BlockTarget::Artist { id }, false) => sets.artists.remove(id),
        };
    }

    /// Retire les titres bloqués et ceux d'un artiste bloqué (principal ou invité).
    fn retain(&self, tracks: &mut Vec<Track>) {
        if let Ok(guard) = self.sets.lock()
            && let Some(sets) = guard.as_ref()
        {
            tracks.retain(|t| sets.allows(t));
        }
    }

    pub fn clear(&self) {
        if let Ok(mut guard) = self.sets.lock() {
            *guard = None;
        }
    }
}

impl TidalClient {
    /// Les deux listes, du blocage le plus récent au plus ancien ; met à jour les ensembles.
    pub async fn blocked(&self) -> Result<Blocked> {
        let user = self.user_id().await?;
        let (tracks_path, artists_path) = (
            format!("users/{user}/blocks/tracks"),
            format!("users/{user}/blocks/artists"),
        );
        let (tracks, artists) = tokio::try_join!(
            self.get_all::<RawFavorite<RawTrack>>(&tracks_path, &[], PAGE_SIZE, BLOCKS_MAX),
            self.get_all::<RawFavorite<RawArtistFull>>(&artists_path, &[], PAGE_SIZE, BLOCKS_MAX),
        )?;
        let blocked = Blocked {
            tracks: tracks
                .into_iter()
                .filter_map(|f| f.item.into_track())
                .collect(),
            artists: artists
                .into_iter()
                .map(|f| ArtistSummary::from(f.item))
                .collect(),
        };
        self.blocks.load(&blocked);
        Ok(blocked)
    }

    pub async fn set_blocked(&self, target: &BlockTarget, blocked: bool) -> Result<()> {
        let user = self.user_id().await?;
        let (kind, field, id) = match target {
            BlockTarget::Track { id } => ("tracks", "trackId", id.0),
            BlockTarget::Artist { id } => ("artists", "artistId", *id),
        };
        if blocked {
            let form = [(field, id.to_string())];
            let path = format!("users/{user}/blocks/{kind}");
            self.write(Method::POST, API_V1, &path, &[], Some(&form), None)
                .await?;
        } else {
            let path = format!("users/{user}/blocks/{kind}/{id}");
            self.write(Method::DELETE, API_V1, &path, &[], None, None)
                .await?;
        }
        self.blocks.set(target, blocked);
        Ok(())
    }

    /// `tracks` sans ce qui est bloqué. Les listes sont lues au premier appel ; si elles
    /// sont indisponibles, rien n'est retiré (les radios du service filtrent déjà) et la
    /// lecture sera retentée au prochain appel.
    pub(super) async fn without_blocked(&self, mut tracks: Vec<Track>) -> Vec<Track> {
        if !self.blocks.loaded()
            && let Err(e) = self.blocked().await
        {
            tracing::warn!("titres et artistes bloqués illisibles : {e}");
        }
        self.blocks.retain(&mut tracks);
        tracks
    }
}

#[cfg(test)]
mod tests {
    use super::{Blocks, Sets};
    use crate::tidal::models::{AlbumRef, ArtistRef, BlockTarget, Blocked, Track, TrackId};

    fn track(id: u64, artists: &[u64]) -> Track {
        Track {
            id: TrackId(id),
            title: format!("t{id}"),
            version: None,
            duration: 200,
            artists: artists
                .iter()
                .map(|&id| ArtistRef {
                    id,
                    name: format!("a{id}"),
                })
                .collect(),
            album: AlbumRef {
                id: 1,
                title: "album".into(),
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

    fn ids(tracks: &[Track]) -> Vec<u64> {
        tracks.iter().map(|t| t.id.0).collect()
    }

    #[test]
    fn nothing_is_removed_before_loading() {
        let blocks = Blocks::default();
        blocks.set(&BlockTarget::Track { id: TrackId(1) }, true);
        let mut tracks = vec![track(1, &[10])];
        blocks.retain(&mut tracks);
        assert_eq!(ids(&tracks), [1]);
    }

    #[test]
    fn blocked_tracks_and_artists_are_removed() {
        let blocks = Blocks::default();
        blocks.load(&Blocked {
            tracks: vec![track(1, &[10])],
            artists: vec![],
        });
        blocks.set(&BlockTarget::Artist { id: 20 }, true);
        // Artiste invité bloqué : le titre part aussi.
        let mut tracks = vec![track(1, &[10]), track(2, &[10, 20]), track(3, &[30])];
        blocks.retain(&mut tracks);
        assert_eq!(ids(&tracks), [3]);

        blocks.set(&BlockTarget::Track { id: TrackId(1) }, false);
        blocks.set(&BlockTarget::Artist { id: 20 }, false);
        let mut tracks = vec![track(1, &[10]), track(2, &[10, 20])];
        blocks.retain(&mut tracks);
        assert_eq!(ids(&tracks), [1, 2]);
    }

    #[test]
    fn clear_forgets_everything() {
        let blocks = Blocks::default();
        blocks.load(&Blocked::default());
        assert!(blocks.loaded());
        blocks.clear();
        assert!(!blocks.loaded());
        assert!(Sets::default().allows(&track(1, &[1])));
    }
}
