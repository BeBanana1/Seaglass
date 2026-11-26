//! Favoris (#4, #81) et mixes enregistrés (#204) : lecture de tous les identifiants,
//! ajout et retrait.

use reqwest::Method;
use serde::Deserialize;

use super::super::api::{API_V1, API_V2};
use super::super::models::{LikeTarget, LikedIds, TrackId};
use super::super::{Result, TidalClient};

impl TidalClient {
    /// Identifiants de tous les favoris, chargés au premier appel puis tenus à jour.
    pub async fn liked_ids(&self) -> Result<LikedIds> {
        if let Some(ids) = self.likes.get() {
            return Ok(ids);
        }
        let user = self.user_id().await?;
        let path = format!("users/{user}/favorites/ids");
        let (raw, mixes) = tokio::try_join!(
            self.get::<RawFavoriteIds>(&path, &[]),
            self.favorite_mix_ids(),
        )?;
        let mut ids = raw.into_ids();
        ids.mixes = mixes;
        self.likes.load(ids);
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
            LikeTarget::Mix { id } => return self.set_mix_liked(id, liked).await,
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

    /// Mixes (#204) : PUT v2 `favorites/mixes/add?mixIds=…` ou `…/remove?mixIds=…`, paramètres
    /// en query, sans corps. Le retrait d'un mix absent répond aussi 200.
    async fn set_mix_liked(&self, id: &str, liked: bool) -> Result<()> {
        let (path, query) = if liked {
            (
                "favorites/mixes/add",
                vec![
                    ("mixIds", id.to_owned()),
                    ("onArtifactNotFound", "FAIL".to_owned()),
                ],
            )
        } else {
            ("favorites/mixes/remove", vec![("mixIds", id.to_owned())])
        };
        self.write(Method::PUT, API_V2, path, &query, None, None)
            .await?;
        self.likes
            .set(&LikeTarget::Mix { id: id.to_owned() }, liked);
        Ok(())
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
            mixes: Vec::new(),
        }
    }
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
}
