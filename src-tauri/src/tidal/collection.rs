//! Écritures dans la bibliothèque de l'utilisateur : favoris (#4, #81) et playlists.
//! Vérifiées sur un vrai compte par `cargo test live_collection -- --ignored --nocapture`.
//!
//! - Favoris (#4, #81) : GET v1 `users/{id}/favorites/ids` (tous les identifiants, en
//!   chaînes), POST `users/{id}/favorites/{tracks,albums,artists,playlists}` (`trackIds`,
//!   `albumIds`, `artistIds`, `uuids`), DELETE `…/<type>/{id}`. Mixes (#204) : GET v2
//!   `favorites/mixes/ids`, PUT v2 `favorites/mixes/add` / `remove` (`mixIds` en query).
//! - Playlist : création PUT v2 `my-collection/playlists/folders/create-playlist`,
//!   suppression PUT v2 `my-collection/playlists/folders/remove` (`trns=trn:playlist:<uuid>`).
//! - Titres d'une playlist : POST v1 `playlists/{uuid}/items` (réponse `addedItemIds`),
//!   DELETE `…/items/{index}`, POST `…/items/{index}` (`toIndex`, déplacement, #201), avec
//!   l'ETag de `playlists/{uuid}` en `If-None-Match`. Titre et description : POST v1
//!   `playlists/{uuid}` (#201).
//! - Écritures par lots de la sélection multiple (#332) : `batch.rs`.

use reqwest::Method;
use serde::Deserialize;

use super::api::{API_V1, API_V2, Call};
use super::models::{LikeTarget, LikedIds, PlaylistSummary, TrackId};
use super::raw::RawPage;
use super::versions;
use super::{Result, TidalClient, TidalError};

/// Tentatives d'une modification de playlist refusée pour ETag périmé.
const STALE_ATTEMPTS: usize = 3;
const STALE_DELAY: std::time::Duration = std::time::Duration::from_millis(400);

/// Relance une modification (lecture de l'ETag comprise) refusée par un 412. Juste après
/// une écriture, `playlists/{uuid}` renvoie parfois encore l'ancien ETag (vu le 2026-11-05,
/// #201 : deux écritures de suite échouaient) ; quelques centaines de ms suffisent. Macro
/// plutôt que fonction prenant une fermeture asynchrone : celle-ci faisait échouer
/// l'inférence de `Send` des commandes Tauri.
macro_rules! retry_stale {
    ($op:expr) => {{
        let mut attempt = 1;
        loop {
            match $op {
                Err(TidalError::Api { status: 412, .. }) if attempt < STALE_ATTEMPTS => {
                    tracing::debug!("ETag périmé, nouvelle tentative ({attempt})");
                    tokio::time::sleep(STALE_DELAY).await;
                    attempt += 1;
                }
                other => break other.map_err(stale_to_conflict),
            }
        }
    }};
}

// Après la macro : `batch` s'en sert.
mod batch;

/// Lectures de l'ETag en attendant une réplique à jour (`fresh_etag`, `settle`).
const VERSION_ATTEMPTS: usize = 6;
const VERSION_DELAY: std::time::Duration = std::time::Duration::from_millis(250);

/// Titres envoyés par requête d'ajout (`limit` de l'API).
const ADD_CHUNK: usize = 100;

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

    /// Supprime une playlist de l'utilisateur (#201).
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
            let ids: Vec<String> = chunk.iter().map(ToString::to_string).collect();
            let form = [
                ("trackIds", ids.join(",")),
                ("onArtifactNotFound", "FAIL".to_owned()),
                ("onDupes", "SKIP".to_owned()),
            ];
            // Chaque ajout change l'ETag : il est relu avant chaque requête.
            let (resp, etag) = retry_stale!(
                async {
                    let etag = self.fresh_etag(uuid).await?;
                    self.write_raw(Method::POST, API_V1, &path, &[], Some(&form), Some(&etag))
                        .await
                        .map(|r| (r, etag))
                }
                .await
            )?;
            self.settle(uuid, &etag).await;
            let resp: RawAdded = resp
                .json()
                .await
                .map_err(|e| TidalError::Decode(format!("{path} : {e}")))?;
            added += resp.added_item_ids.len();
        }
        Ok(added)
    }

    /// Remet un titre à la position `index` (0 = premier) : annulation d'un retrait (#349).
    /// `onDupes=ADD` : le titre revient même s'il figure ailleurs dans la playlist.
    /// Renvoie `false` si rien n'a été ajouté.
    pub async fn insert_in_playlist(&self, uuid: &str, index: usize, id: TrackId) -> Result<bool> {
        self.playlists.remove(uuid);
        let path = format!("playlists/{uuid}/items");
        let form = [
            ("trackIds", id.to_string()),
            ("onArtifactNotFound", "FAIL".to_owned()),
            ("onDupes", "ADD".to_owned()),
            ("toIndex", index.to_string()),
        ];
        let (resp, etag) = retry_stale!(
            async {
                let etag = self.fresh_etag(uuid).await?;
                self.write_raw(Method::POST, API_V1, &path, &[], Some(&form), Some(&etag))
                    .await
                    .map(|r| (r, etag))
            }
            .await
        )?;
        self.settle(uuid, &etag).await;
        let resp: RawAdded = resp
            .json()
            .await
            .map_err(|e| TidalError::Decode(format!("{path} : {e}")))?;
        Ok(!resp.added_item_ids.is_empty())
    }

    /// Retire le titre à cette position (0 = premier), après avoir vérifié que c'est bien
    /// `expected` : la playlist a pu changer ailleurs depuis son affichage.
    pub async fn remove_from_playlist(
        &self,
        uuid: &str,
        index: usize,
        expected: TrackId,
    ) -> Result<()> {
        let path = format!("playlists/{uuid}/items/{index}");
        let etag = retry_stale!(
            async {
                let etag = self.checked_etag(uuid, index, expected).await?;
                self.write_raw(Method::DELETE, API_V1, &path, &[], None, Some(&etag))
                    .await
                    .map(|_| etag)
            }
            .await
        )?;
        self.settle(uuid, &etag).await;
        Ok(())
    }

    /// Déplace le titre `from` pour qu'il finisse à la position `to` (#201) : POST v1
    /// `playlists/{uuid}/items/{from}` avec `toIndex` **en formulaire** (en query, il est
    /// ignoré sans erreur). Au-delà de la fin, le titre va à la fin.
    pub async fn move_in_playlist(
        &self,
        uuid: &str,
        from: usize,
        to: usize,
        expected: TrackId,
    ) -> Result<()> {
        let path = format!("playlists/{uuid}/items/{from}");
        let form = [("toIndex", to.to_string())];
        let etag = retry_stale!(
            async {
                let etag = self.checked_etag(uuid, from, expected).await?;
                self.write_raw(Method::POST, API_V1, &path, &[], Some(&form), Some(&etag))
                    .await
                    .map(|_| etag)
            }
            .await
        )?;
        self.settle(uuid, &etag).await;
        Ok(())
    }

    /// Renomme et change la description (#201) : POST v1 `playlists/{uuid}` (formulaire
    /// `title`, `description`), visible aussitôt.
    pub async fn edit_playlist(&self, uuid: &str, title: &str, description: &str) -> Result<()> {
        self.playlists.remove(uuid);
        let path = format!("playlists/{uuid}");
        let form = [
            ("title", title.to_owned()),
            ("description", description.to_owned()),
        ];
        let etag = retry_stale!(
            async {
                let etag = self.fresh_etag(uuid).await?;
                self.write_raw(Method::POST, API_V1, &path, &[], Some(&form), Some(&etag))
                    .await
                    .map(|_| etag)
            }
            .await
        )?;
        self.settle(uuid, &etag).await;
        Ok(())
    }

    /// Copie gardée effacée, puis ETag de la playlist, après avoir vérifié que le titre à
    /// `index` est bien `expected`. ETag lu avant la vérification : une modification entre
    /// les deux donne un 412.
    async fn checked_etag(&self, uuid: &str, index: usize, expected: TrackId) -> Result<String> {
        self.playlists.remove(uuid);
        let etag = self.fresh_etag(uuid).await?;
        let query = [("offset", index.to_string()), ("limit", "1".to_owned())];
        let page: RawPage<RawItem> = self.get(&format!("playlists/{uuid}/items"), &query).await?;
        if page.items.first().map(|i| i.item.id) != Some(expected) {
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

    /// ETag à envoyer avec une modification : jamais plus ancien que la dernière version vue
    /// après une écriture de l'app (`versions.rs`) ; une réplique en retard est relue.
    async fn fresh_etag(&self, uuid: &str) -> Result<String> {
        for _ in 0..VERSION_ATTEMPTS {
            let etag = self.playlist_etag(uuid).await?;
            if !self.versions.is_stale(uuid, &etag) {
                return Ok(etag);
            }
            tracing::debug!("ETag en retard sur la dernière écriture, relu");
            tokio::time::sleep(VERSION_DELAY).await;
        }
        Err(stale_to_conflict(TidalError::Api {
            status: 412,
            message: String::new(),
        }))
    }

    /// Après une écriture envoyée avec `before` : attend de voir la nouvelle version et la
    /// retient, pour que la modification suivante ne parte pas d'un état plus ancien.
    async fn settle(&self, uuid: &str, before: &str) {
        for _ in 0..VERSION_ATTEMPTS {
            match self.playlist_etag(uuid).await {
                Ok(after) if versions::is_newer(&after, before) => {
                    self.versions.note(uuid, &after);
                    return;
                }
                Ok(_) => tokio::time::sleep(VERSION_DELAY).await,
                Err(e) => {
                    tracing::debug!("version après écriture illisible : {e}");
                    return;
                }
            }
        }
        tracing::debug!("nouvelle version de la playlist pas encore visible");
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

    /// Écriture ; un ETag refusé (412) devient `TidalError::Conflict`.
    pub(super) async fn write(
        &self,
        method: Method,
        base: &str,
        path: &str,
        query: &[(&str, String)],
        form: Option<&[(&str, String)]>,
        if_none_match: Option<&str>,
    ) -> Result<reqwest::Response> {
        self.write_raw(method, base, path, query, form, if_none_match)
            .await
            .map_err(stale_to_conflict)
    }

    /// Écriture qui laisse passer le 412, pour `retry_stale`.
    async fn write_raw(
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
            json: None,
            if_none_match,
        })
        .await
    }
}

fn stale_to_conflict(e: TidalError) -> TidalError {
    match e {
        TidalError::Api { status: 412, .. } => TidalError::Conflict(
            crate::i18n::tr(
                "la playlist a été modifiée ailleurs, réessayez",
                "the playlist was changed elsewhere, try again",
            )
            .into(),
        ),
        other => other,
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
