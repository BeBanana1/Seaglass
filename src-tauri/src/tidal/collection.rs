//! Écritures dans la bibliothèque de l'utilisateur : favoris (#4, #81) et playlists.
//! Vérifiées sur un vrai compte par `cargo test live_collection -- --ignored --nocapture`.
//!
//! - Favoris (#4, #81) : GET v1 `users/{id}/favorites/ids` (tous les identifiants, en
//!   chaînes), POST `users/{id}/favorites/{tracks,albums,artists,playlists}` (`trackIds`,
//!   `albumIds`, `artistIds`, `uuids`), DELETE `…/<type>/{id}`.
//! - Playlist : création PUT v2 `my-collection/playlists/folders/create-playlist`,
//!   suppression PUT v2 `my-collection/playlists/folders/remove` (`trns=trn:playlist:<uuid>`).
//! - Titres d'une playlist : POST v1 `playlists/{uuid}/items` (réponse `addedItemIds`),
//!   DELETE `…/items/{index}`, avec l'ETag de `playlists/{uuid}` en `If-None-Match`.

use reqwest::Method;
use serde::Deserialize;

use super::api::{API_V1, API_V2, Call};
use super::models::{LikeTarget, LikedIds, PlaylistSummary, TrackId};
use super::raw::RawPage;
use super::{Result, TidalClient, TidalError};

/// Titres envoyés par requête d'ajout (`limit` de l'API).
const ADD_CHUNK: usize = 100;

impl TidalClient {
    /// Identifiants de tous les favoris, chargés au premier appel puis tenus à jour.
    pub async fn liked_ids(&self) -> Result<LikedIds> {
        if let Some(ids) = self.likes.get() {
            return Ok(ids);
        }
        let user = self.user_id().await?;
        let raw: RawFavoriteIds = self
            .get(&format!("users/{user}/favorites/ids"), &[])
            .await?;
        self.likes.load(raw.into_ids());
        Ok(self.likes.get().unwrap_or_default())
    }

    /// Ajoute aux favoris ou retire : POST `users/{id}/favorites/<type>` (formulaire
    /// `trackIds`, `albumIds`, `artistIds` ou `uuids`), DELETE `…/<type>/<id>`.
    pub async fn set_liked(&self, target: &LikeTarget, liked: bool) -> Result<()> {
        let user = self.user_id().await?;
        let (kind, field, id) = match target {
            LikeTarget::Track { id } => ("tracks", "trackIds", id.to_string()),
            LikeTarget::Album { id } => ("albums", "albumIds", id.to_string()),
            LikeTarget::Artist { id } => ("artists", "artistIds", id.to_string()),
            LikeTarget::Playlist { uuid } => ("playlists", "uuids", uuid.clone()),
        };
        if liked {
            let form = [(field, id), ("onArtifactNotFound", "FAIL".to_owned())];
            let path = format!("users/{user}/favorites/{kind}");
            self.write(Method::POST, API_V1, &path, &[], Some(&form), None)
                .await?;
        } else {
            let path = format!("users/{user}/favorites/{kind}/{id}");
            self.write(Method::DELETE, API_V1, &path, &[], None, None)
                .await?;
        }
        self.likes.set(target, liked);
        Ok(())
    }

    /// Playlists de l'utilisateur, les seules qu'il peut modifier.
    pub async fn own_playlists(&self) -> Result<Vec<PlaylistSummary>> {
        let mut playlists = self.library_playlists().await?.items;
        playlists.retain(|p| p.mine);
        Ok(playlists)
    }

    /// Crée une playlist à la racine de la bibliothèque et renvoie son uuid.
    pub async fn create_playlist(&self, title: &str, description: &str) -> Result<String> {
        let query = [
            ("name", title.to_owned()),
            ("description", description.to_owned()),
            ("folderId", "root".to_owned()),
        ];
        let path = "my-collection/playlists/folders/create-playlist";
        let created: RawCreated = self
            .write(Method::PUT, API_V2, path, &query, None, None)
            .await?
            .json()
            .await
            .map_err(|e| TidalError::Decode(format!("{path} : {e}")))?;
        Ok(created.data.uuid)
    }

    // Pas encore proposé dans l'UI : sert au nettoyage du test réel.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn delete_playlist(&self, uuid: &str) -> Result<()> {
        self.playlists.remove(uuid);
        let query = [("trns", format!("trn:playlist:{uuid}"))];
        let path = "my-collection/playlists/folders/remove";
        self.write(Method::PUT, API_V2, path, &query, None, None)
            .await
            .map(drop)
    }

    /// Ajoute des titres à la fin d'une playlist, dans l'ordre, sans doublon.
    /// Renvoie le nombre de titres réellement ajoutés (0 s'ils y étaient tous).
    pub async fn add_to_playlist(&self, uuid: &str, ids: &[TrackId]) -> Result<usize> {
        // Copie gardée effacée avant d'écrire : même un échec partiel la rend fausse.
        self.playlists.remove(uuid);
        let path = format!("playlists/{uuid}/items");
        let mut added = 0;
        for chunk in ids.chunks(ADD_CHUNK) {
            // Chaque ajout change l'ETag : il est relu avant chaque requête.
            let etag = self.playlist_etag(uuid).await?;
            let ids: Vec<String> = chunk.iter().map(ToString::to_string).collect();
            let form = [
                ("trackIds", ids.join(",")),
                ("onArtifactNotFound", "FAIL".to_owned()),
                ("onDupes", "SKIP".to_owned()),
            ];
            let resp: RawAdded = self
                .write(Method::POST, API_V1, &path, &[], Some(&form), Some(&etag))
                .await?
                .json()
                .await
                .map_err(|e| TidalError::Decode(format!("{path} : {e}")))?;
            added += resp.added_item_ids.len();
        }
        Ok(added)
    }

    /// Retire le titre à cette position (0 = premier), après avoir vérifié que c'est bien
    /// `expected` : la playlist a pu changer ailleurs depuis son affichage.
    pub async fn remove_from_playlist(
        &self,
        uuid: &str,
        index: usize,
        expected: TrackId,
    ) -> Result<()> {
        self.playlists.remove(uuid);
        // ETag lu avant la vérification : une modification entre les deux donne un 412.
        let etag = self.playlist_etag(uuid).await?;
        let query = [("offset", index.to_string()), ("limit", "1".to_owned())];
        let page: RawPage<RawItem> = self.get(&format!("playlists/{uuid}/items"), &query).await?;
        if page.items.first().map(|i| i.item.id) != Some(expected) {
            return Err(TidalError::Conflict(
                crate::i18n::tr(
                    "la playlist a changé, rouvrez-la avant de retirer ce titre",
                    "the playlist has changed, reopen it before removing this track",
                )
                .into(),
            ));
        }
        let path = format!("playlists/{uuid}/items/{index}");
        self.write(Method::DELETE, API_V1, &path, &[], None, Some(&etag))
            .await
            .map(drop)
    }

    /// Version courante de la playlist, exigée par chaque modification.
    pub(super) async fn playlist_etag(&self, uuid: &str) -> Result<String> {
        let path = format!("playlists/{uuid}");
        let resp = self.call(&Call::get(API_V1, &path, &[])).await?;
        resp.headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
            .ok_or_else(|| TidalError::Decode(format!("{path} : pas d'ETag")))
    }

    pub(super) async fn write(
        &self,
        method: Method,
        base: &str,
        path: &str,
        query: &[(&str, String)],
        form: Option<&[(&str, String)]>,
        if_none_match: Option<&str>,
    ) -> Result<reqwest::Response> {
        let resp = self
            .call(&Call {
                method,
                base,
                path,
                query,
                form,
                if_none_match,
            })
            .await;
        match resp {
            Err(TidalError::Api { status: 412, .. }) => Err(TidalError::Conflict(
                crate::i18n::tr(
                    "la playlist a été modifiée ailleurs, réessayez",
                    "the playlist was changed elsewhere, try again",
                )
                .into(),
            )),
            other => other,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "UPPERCASE")]
struct RawFavoriteIds {
    #[serde(default)]
    track: Vec<String>,
    #[serde(default)]
    album: Vec<String>,
    #[serde(default)]
    artist: Vec<String>,
    #[serde(default)]
    playlist: Vec<String>,
}

impl RawFavoriteIds {
    /// Identifiants en chaînes ; un identifiant numérique illisible est ignoré.
    fn into_ids(self) -> LikedIds {
        let numbers = |ids: Vec<String>| -> Vec<u64> {
            ids.iter().filter_map(|id| id.parse().ok()).collect()
        };
        LikedIds {
            tracks: numbers(self.track).into_iter().map(TrackId).collect(),
            albums: numbers(self.album),
            artists: numbers(self.artist),
            playlists: self.playlist,
        }
    }
}

#[derive(Deserialize)]
struct RawCreated {
    data: RawCreatedPlaylist,
}

#[derive(Deserialize)]
struct RawCreatedPlaylist {
    uuid: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAdded {
    #[serde(default)]
    added_item_ids: Vec<u64>,
}

#[derive(Deserialize)]
struct RawItem {
    item: RawItemId,
}

#[derive(Deserialize)]
struct RawItemId {
    id: TrackId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn favorite_ids_are_strings() {
        let raw: RawFavoriteIds = serde_json::from_str(
            r#"{"ARTIST":["1134"],"TRACK":["156749594","x"],"PLAYLIST":["4ba30155-d807-446f-9e53-b67520ccfe83"],"VIDEO":[]}"#,
        )
        .unwrap();
        let ids = raw.into_ids();
        assert_eq!(ids.tracks, [TrackId(156_749_594)]);
        assert_eq!(ids.artists, [1134]);
        assert!(ids.albums.is_empty());
        assert_eq!(ids.playlists, ["4ba30155-d807-446f-9e53-b67520ccfe83"]);
    }

    #[test]
    fn added_ids_empty_on_duplicate() {
        let raw: RawAdded =
            serde_json::from_str(r#"{"lastUpdated":1793366927853,"addedItemIds":[]}"#).unwrap();
        assert!(raw.added_item_ids.is_empty());
        let raw: RawAdded =
            serde_json::from_str(r#"{"lastUpdated":1,"addedItemIds":[156749594,156749595]}"#)
                .unwrap();
        assert_eq!(raw.added_item_ids.len(), 2);
    }
}
