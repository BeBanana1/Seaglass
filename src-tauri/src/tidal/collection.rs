//! Écritures dans la bibliothèque de l'utilisateur (#4) : titres likés et playlists.
//! Vérifiées sur un vrai compte par `cargo test live_collection -- --ignored --nocapture`.
//!
//! - Like : POST v1 `users/{id}/favorites/tracks` (`trackIds`), DELETE `…/tracks/{id}`.
//! - Playlist : création PUT v2 `my-collection/playlists/folders/create-playlist`,
//!   suppression PUT v2 `my-collection/playlists/folders/remove` (`trns=trn:playlist:<uuid>`).
//! - Titres d'une playlist : POST v1 `playlists/{uuid}/items`, DELETE `…/items/{index}`,
//!   avec l'ETag de `playlists/{uuid}` en `If-None-Match`.

// Vérifié, pas encore branché sur des commandes : l'UI de #4 viendra ensuite.
#![cfg_attr(not(test), allow(dead_code))]

use reqwest::Method;
use serde::Deserialize;

use super::api::{API_V1, API_V2, Call};
use super::models::TrackId;
use super::{Result, TidalClient, TidalError};

impl TidalClient {
    pub async fn like_track(&self, id: TrackId) -> Result<()> {
        let user = self.user_id().await?;
        let form = [
            ("trackIds", id.to_string()),
            ("onArtifactNotFound", "FAIL".to_owned()),
        ];
        let path = format!("users/{user}/favorites/tracks");
        self.write(Method::POST, API_V1, &path, &[], Some(&form), None)
            .await
            .map(drop)
    }

    pub async fn unlike_track(&self, id: TrackId) -> Result<()> {
        let user = self.user_id().await?;
        let path = format!("users/{user}/favorites/tracks/{id}");
        self.write(Method::DELETE, API_V1, &path, &[], None, None)
            .await
            .map(drop)
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

    pub async fn delete_playlist(&self, uuid: &str) -> Result<()> {
        let query = [("trns", format!("trn:playlist:{uuid}"))];
        let path = "my-collection/playlists/folders/remove";
        self.write(Method::PUT, API_V2, path, &query, None, None)
            .await
            .map(drop)
    }

    /// Ajoute des titres à la fin d'une playlist, sans doublon.
    pub async fn add_to_playlist(&self, uuid: &str, ids: &[TrackId]) -> Result<()> {
        let etag = self.playlist_etag(uuid).await?;
        let ids: Vec<String> = ids.iter().map(ToString::to_string).collect();
        let form = [
            ("trackIds", ids.join(",")),
            ("onArtifactNotFound", "FAIL".to_owned()),
            ("onDupes", "SKIP".to_owned()),
        ];
        let path = format!("playlists/{uuid}/items");
        self.write(Method::POST, API_V1, &path, &[], Some(&form), Some(&etag))
            .await
            .map(drop)
    }

    /// Retire le titre à cette position (0 = premier) d'une playlist.
    pub async fn remove_from_playlist(&self, uuid: &str, index: usize) -> Result<()> {
        let etag = self.playlist_etag(uuid).await?;
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
        self.call(&Call {
            method,
            base,
            path,
            query,
            form,
            if_none_match,
        })
        .await
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
