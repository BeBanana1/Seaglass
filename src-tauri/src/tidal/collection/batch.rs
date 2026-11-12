//! Écritures par lots pour la sélection multiple (#332), vérifiées par
//! `cargo test live_batch -- --ignored --nocapture`.
//!
//! - Favoris : POST v1 `users/{id}/favorites/tracks` accepte plusieurs `trackIds` séparés
//!   par des virgules ; DELETE `…/favorites/tracks/{a,b,c}` retire plusieurs titres d'un coup.
//! - Playlist : DELETE v1 `playlists/{uuid}/items/{i,j,k}` retire plusieurs positions en une
//!   requête (une seule version de la playlist, donc un seul ETag).

use reqwest::Method;

use super::super::api::API_V1;
use super::super::models::{LikeTarget, TrackId};
use super::super::raw::RawPage;
use super::super::{Result, TidalClient, TidalError};
// `retry_stale!` se sert des trois derniers.
use super::{RawItem, STALE_ATTEMPTS, STALE_DELAY, stale_to_conflict};

/// Titres par requête (favoris, positions retirées) : l'identifiant finit dans le chemin ou
/// le formulaire, on reste loin des limites d'URL.
const CHUNK: usize = 100;
/// Titres lus par page pour vérifier les positions avant un retrait.
const PAGE: usize = 100;

impl TidalClient {
    /// Ajoute ou retire plusieurs titres des favoris. Les titres déjà dans l'état demandé
    /// sont ignorés par l'appelant ; ici tout est envoyé.
    pub async fn set_tracks_liked(&self, ids: &[TrackId], liked: bool) -> Result<()> {
        let user = self.user_id().await?;
        for chunk in ids.chunks(CHUNK) {
            let joined = join(chunk);
            if liked {
                let form = [
                    ("trackIds", joined),
                    ("onArtifactNotFound", "FAIL".to_owned()),
                ];
                let path = format!("users/{user}/favorites/tracks");
                self.write(Method::POST, API_V1, &path, &[], Some(&form), None)
                    .await?;
            } else {
                let path = format!("users/{user}/favorites/tracks/{joined}");
                self.write(Method::DELETE, API_V1, &path, &[], None, None)
                    .await?;
            }
            for id in chunk {
                self.likes.set(&LikeTarget::Track { id: *id }, liked);
            }
        }
        Ok(())
    }

    /// Retire plusieurs titres d'une playlist, désignés par leur position (0 = premier) et
    /// le titre attendu à cette position : tout est refusé si l'un d'eux a changé.
    pub async fn remove_many_from_playlist(
        &self,
        uuid: &str,
        items: &[(usize, TrackId)],
    ) -> Result<()> {
        let mut items = items.to_vec();
        items.sort_unstable_by_key(|(index, _)| *index);
        items.dedup_by_key(|(index, _)| *index);
        // Du bas vers le haut, par lots : un lot retiré ne décale pas les positions des
        // lots suivants, plus haut dans la playlist.
        for chunk in items.rchunks(CHUNK) {
            let indices: Vec<String> = chunk.iter().map(|(i, _)| i.to_string()).collect();
            let path = format!("playlists/{uuid}/items/{}", indices.join(","));
            let etag = retry_stale!(
                async {
                    let etag = self.checked_etag_many(uuid, chunk).await?;
                    self.write_raw(Method::DELETE, API_V1, &path, &[], None, Some(&etag))
                        .await
                        .map(|_| etag)
                }
                .await
            )?;
            self.settle(uuid, &etag).await;
        }
        Ok(())
    }

    /// Comme `checked_etag`, pour plusieurs positions triées : les titres sont relus par
    /// pages entre la première et la dernière position.
    async fn checked_etag_many(&self, uuid: &str, items: &[(usize, TrackId)]) -> Result<String> {
        self.playlists.remove(uuid);
        let etag = self.fresh_etag(uuid).await?;
        let (Some(first), Some(last)) = (items.first(), items.last()) else {
            return Ok(etag);
        };
        let path = format!("playlists/{uuid}/items");
        let mut found: Vec<TrackId> = Vec::new();
        let mut offset = first.0;
        while offset <= last.0 {
            let query = [("offset", offset.to_string()), ("limit", PAGE.to_string())];
            let page: RawPage<RawItem> = self.get(&path, &query).await?;
            if page.items.is_empty() {
                break;
            }
            offset += page.items.len();
            found.extend(page.items.into_iter().map(|i| i.item.id));
        }
        let at = |index: usize| found.get(index - first.0).copied();
        if items.iter().any(|(index, id)| at(*index) != Some(*id)) {
            return Err(TidalError::Conflict(
                crate::i18n::tr(
                    "la playlist a changé, rouvrez-la avant de la modifier",
                    "the playlist has changed, reopen it before editing it",
                )
                .into(),
            ));
        }
        Ok(etag)
    }

    /// Annulation d'un retrait par lot : chaque titre revient à sa position, de la plus
    /// petite à la plus grande (chaque remise rétablit les positions des suivantes).
    /// Renvoie le nombre de titres remis.
    pub async fn restore_many_to_playlist(
        &self,
        uuid: &str,
        items: &[(usize, TrackId)],
    ) -> Result<usize> {
        let mut items = items.to_vec();
        items.sort_unstable_by_key(|(index, _)| *index);
        let mut restored = 0;
        for (index, id) in items {
            if self.insert_in_playlist(uuid, index, id).await? {
                restored += 1;
            }
        }
        Ok(restored)
    }
}

fn join(ids: &[TrackId]) -> String {
    ids.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}
