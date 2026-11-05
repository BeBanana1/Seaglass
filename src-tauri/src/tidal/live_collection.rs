//! Écritures dans la bibliothèque sur l'API réelle (#4, #81, #201, #204), ignorées par défaut.
//!
//! `cargo test live_collection -- --ignored --nocapture` : **modifie le compte**. Seuls des
//! éléments absents des favoris sont ajoutés puis retirés : aucun favori existant n'est
//! touché, et le nettoyage a lieu même si une vérification échoue.

use super::TidalClient;
use super::live_tests::{ALBUM_ID, client};
use super::models::{LikeTarget, LikedIds, MediaItem, TrackId};

type Check = Result<(), String>;

fn err(e: super::TidalError) -> String {
    e.to_string()
}

/// Like puis retrait d'un titre, d'un album, d'un artiste, d'une playlist d'un autre
/// utilisateur et d'un mix, absents des favoris ; puis playlist jetable créée, remplie,
/// réordonnée, renommée, vidée et supprimée.
#[tokio::test]
#[ignore = "réseau et session TIDAL requis, modifie la bibliothèque"]
async fn live_collection() {
    let client = client().await;
    let album = client.album(ALBUM_ID).await.unwrap();
    // `favorites/ids` contient tous les titres likés, indisponibles compris (451 contre 426
    // affichés le 2026-10-30) : la liste des favoris en est un sous-ensemble.
    let liked = client.liked_ids().await.unwrap();
    let favorites = favorite_track_ids(&client).await;
    println!(
        "likes : {} titres, {} albums, {} artistes, {} playlists ; {} titres favoris",
        liked.tracks.len(),
        liked.albums.len(),
        liked.artists.len(),
        liked.playlists.len(),
        favorites.len()
    );
    assert!(favorites.iter().all(|id| liked.tracks.contains(id)));
    let fresh: Vec<TrackId> = album
        .tracks
        .iter()
        .map(|t| t.id)
        .filter(|id| !liked.tracks.contains(id))
        .take(4)
        .collect();
    assert_eq!(fresh.len(), 4, "pas assez de titres hors favoris");

    let targets = fresh_targets(&client, &liked).await;
    let album_target = targets[0].clone();
    let mut results = Vec::new();
    for target in [vec![LikeTarget::Track { id: fresh[0] }], targets].concat() {
        // Jamais de retrait d'un favori existant : un élément déjà présent est sauté.
        if contains(&server_ids(&client).await.unwrap(), &target) {
            results.push((target, Err("déjà dans les favoris".to_owned())));
            continue;
        }
        let added = check_added(&client, &target).await;
        // Retrait systématique : l'élément n'était pas dans les favoris avant le test.
        let removed = client.set_liked(&target, false).await.map_err(err);
        let absent = check_absent(&client, &target).await;
        println!("{target:?} : ajout {added:?}, retrait {removed:?}, absent {absent:?}");
        results.push((target, added.and(removed).and(absent)));
    }
    // Retrait d'un favori déjà retiré (ailleurs, par exemple) : idempotent ou erreur ?
    if !contains(&server_ids(&client).await.unwrap(), &album_target) {
        let twice = client.set_liked(&album_target, false).await;
        println!("second retrait de l'album : {twice:?}");
    }

    let uuid = client
        .create_playlist(
            "Seaglass test (à supprimer)",
            "Créée par un test, supprimée aussitôt",
        )
        .await
        .unwrap();
    println!("playlist créée : {uuid}");
    let playlist = check_playlist(&client, &uuid, &fresh).await;
    let deleted = client.delete_playlist(&uuid).await;
    let gone = client.playlist(&uuid).await;
    println!(
        "après suppression : {:?}",
        gone.as_ref().map(|p| &p.playlist.title)
    );

    for (target, result) in results {
        assert!(result.is_ok(), "{target:?} : {result:?}");
    }
    playlist.unwrap();
    deleted.unwrap();
    assert!(gone.is_err(), "playlist encore lisible après suppression");
}

/// Un album, un artiste, une playlist d'un autre utilisateur et un mix absents des favoris.
async fn fresh_targets(client: &TidalClient, liked: &LikedIds) -> Vec<LikeTarget> {
    let search = client.search("interstellar", 10).await.unwrap();
    let album = search
        .albums
        .iter()
        .find(|a| !liked.albums.contains(&a.id))
        .expect("aucun album hors favoris");
    let artist = search
        .artists
        .iter()
        .find(|a| !liked.artists.contains(&a.id))
        .expect("aucun artiste hors favoris");
    let own = client.own_playlists().await.unwrap();
    let playlist = search
        .playlists
        .iter()
        .find(|p| !liked.playlists.contains(&p.uuid) && !own.iter().any(|o| o.uuid == p.uuid))
        .expect("aucune playlist hors favoris");
    println!(
        "cibles : album {} « {} », artiste {} « {} », playlist {} « {} »",
        album.id, album.title, artist.id, artist.name, playlist.uuid, playlist.title
    );
    // Un mix de l'accueil absent des favoris (#204).
    let home = client.home().await.unwrap();
    let mix = home
        .iter()
        .flat_map(|row| &row.items)
        .find_map(|item| match item {
            MediaItem::Mix(m) if !liked.mixes.contains(&m.id) => Some(m.clone()),
            _ => None,
        })
        .expect("aucun mix hors favoris");
    println!("mix {} « {} »", mix.id, mix.title);
    vec![
        LikeTarget::Album { id: album.id },
        LikeTarget::Artist { id: artist.id },
        LikeTarget::Playlist {
            uuid: playlist.uuid.clone(),
        },
        LikeTarget::Mix { id: mix.id },
    ]
}

/// Relu sur le serveur, sans l'ensemble en mémoire.
async fn server_ids(client: &TidalClient) -> Result<LikedIds, String> {
    client.likes.clear();
    client.liked_ids().await.map_err(err)
}

fn contains(ids: &LikedIds, target: &LikeTarget) -> bool {
    match target {
        LikeTarget::Track { id } => ids.tracks.contains(id),
        LikeTarget::Album { id } => ids.albums.contains(id),
        LikeTarget::Artist { id } => ids.artists.contains(id),
        LikeTarget::Playlist { uuid } => ids.playlists.contains(uuid),
        LikeTarget::Mix { id } => ids.mixes.contains(id),
    }
}

/// Position de l'élément dans la liste de la bibliothèque (favoris du plus récent au plus
/// ancien), `None` s'il n'y est pas.
async fn library_position(
    client: &TidalClient,
    target: &LikeTarget,
) -> Result<Option<usize>, String> {
    Ok(match target {
        LikeTarget::Track { id } => favorite_track_ids(client)
            .await
            .iter()
            .position(|t| t == id),
        LikeTarget::Album { id } => {
            let albums = client.favorite_albums().await.map_err(err)?.items;
            albums.iter().position(|a| a.id == *id)
        }
        LikeTarget::Artist { id } => {
            let artists = client.favorite_artists().await.map_err(err)?.items;
            artists.iter().position(|a| a.id == *id)
        }
        LikeTarget::Playlist { uuid } => {
            let playlists = client.library_playlists().await.map_err(err)?.items;
            playlists.iter().position(|p| p.uuid == *uuid)
        }
        LikeTarget::Mix { id } => {
            let mixes = client.favorite_mixes().await.map_err(err)?;
            mixes.iter().position(|m| m.id == *id)
        }
    })
}

async fn check_added(client: &TidalClient, target: &LikeTarget) -> Check {
    client.set_liked(target, true).await.map_err(err)?;
    if !contains(&client.liked_ids().await.map_err(err)?, target) {
        return Err("absent des favoris en mémoire".into());
    }
    if !contains(&server_ids(client).await?, target) {
        return Err("absent de favorites/ids".into());
    }
    let position = library_position(client, target).await?;
    println!("ajouté : position {position:?} dans la bibliothèque");
    // Titres, albums et artistes sont triés par date d'ajout ; les playlists et mixes non.
    let expected = !matches!(target, LikeTarget::Playlist { .. } | LikeTarget::Mix { .. });
    match position {
        Some(0) => Ok(()),
        Some(_) if !expected => Ok(()),
        other => Err(format!("position dans la bibliothèque : {other:?}")),
    }
}

async fn check_absent(client: &TidalClient, target: &LikeTarget) -> Check {
    if contains(&client.liked_ids().await.map_err(err)?, target)
        || contains(&server_ids(client).await?, target)
    {
        return Err("encore dans les favoris".into());
    }
    match library_position(client, target).await? {
        None => Ok(()),
        Some(i) => Err(format!("encore dans la bibliothèque (position {i})")),
    }
}

async fn favorite_track_ids(client: &TidalClient) -> Vec<TrackId> {
    let favorites = client.favorite_tracks().await.unwrap().items;
    favorites.into_iter().map(|t| t.track.id).collect()
}

async fn check_playlist(client: &TidalClient, uuid: &str, ids: &[TrackId]) -> Check {
    let tracks = async |uuid: &str| -> Result<Vec<TrackId>, String> {
        let page = client.playlist(uuid).await.map_err(err)?;
        Ok(page.tracks.into_iter().map(|t| t.track.id).collect())
    };
    let own = client.own_playlists().await.map_err(err)?;
    if !own.iter().any(|p| p.uuid == uuid) {
        return Err("playlist créée absente de own_playlists".into());
    }

    let added = client.add_to_playlist(uuid, &ids[..2]).await.map_err(err)?;
    let after_add = tracks(uuid).await?;
    println!("ajout de {added} titres : {after_add:?}");
    if added != 2 || after_add != ids[..2] {
        return Err(format!("ajout : {added}, {after_add:?}"));
    }

    // Doublon (onDupes=SKIP) : ignoré sans erreur, `addedItemIds` vide.
    let dupe = client.add_to_playlist(uuid, &ids[..1]).await;
    let after_dupe = tracks(uuid).await?;
    println!("doublon : {dupe:?}, titres {after_dupe:?}");
    if !matches!(dupe, Ok(0)) || after_dupe != ids[..2] {
        return Err(format!("doublon : {dupe:?}"));
    }

    // ETag périmé : une modification faite entre-temps doit être refusée (412).
    let stale = client.playlist_etag(uuid).await.map_err(err)?;
    client
        .add_to_playlist(uuid, &ids[2..3])
        .await
        .map_err(err)?;
    let form = [
        ("trackIds", ids[3].to_string()),
        ("onArtifactNotFound", "FAIL".to_owned()),
    ];
    let path = format!("playlists/{uuid}/items");
    let stale_write = client
        .write(
            reqwest::Method::POST,
            super::api::API_V1,
            &path,
            &[],
            Some(&form),
            Some(&stale),
        )
        .await;
    println!(
        "ETag périmé : {:?}",
        stale_write.as_ref().map(|r| r.status())
    );
    if !matches!(stale_write, Err(super::TidalError::Conflict(_))) {
        return Err("ETag périmé accepté".into());
    }

    let before = tracks(uuid).await?;
    // Mauvais titre attendu à cette position : refusé sans rien retirer.
    let wrong = client.remove_from_playlist(uuid, 0, before[1]).await;
    if !matches!(wrong, Err(super::TidalError::Conflict(_))) {
        return Err(format!("retrait non vérifié : {wrong:?}"));
    }
    client
        .remove_from_playlist(uuid, 0, before[0])
        .await
        .map_err(err)?;
    let after = tracks(uuid).await?;
    println!("avant retrait : {before:?}, après retrait de l'index 0 : {after:?}");
    if after != before[1..] {
        return Err(format!("retrait : {after:?}"));
    }

    // Déplacement (#201) : le premier titre finit en dernier ; vérification du titre attendu.
    client.add_to_playlist(uuid, &ids[..1]).await.map_err(err)?;
    let before = tracks(uuid).await?;
    let wrong = client.move_in_playlist(uuid, 0, 2, before[1]).await;
    if !matches!(wrong, Err(super::TidalError::Conflict(_))) {
        return Err(format!("déplacement non vérifié : {wrong:?}"));
    }
    let last = before.len() - 1;
    client
        .move_in_playlist(uuid, 0, last, before[0])
        .await
        .map_err(err)?;
    let moved = tracks(uuid).await?;
    println!("avant déplacement : {before:?}, après 0 → {last} : {moved:?}");
    let mut expected = before[1..].to_vec();
    expected.push(before[0]);
    if moved != expected {
        return Err(format!("déplacement : {moved:?}"));
    }
    // Deux déplacements enchaînés (Alt+↓ répété) : l'ETag relu juste après une écriture
    // est parfois l'ancien, `retry_stale` doit absorber le 412.
    client
        .move_in_playlist(uuid, 0, 1, moved[0])
        .await
        .map_err(err)?;
    client
        .move_in_playlist(uuid, 1, 2, moved[0])
        .await
        .map_err(err)?;

    // Titre et description (#201).
    client
        .edit_playlist(uuid, "Seaglass test renommée", "Nouvelle description")
        .await
        .map_err(err)?;
    let page = client.playlist(uuid).await.map_err(err)?;
    println!(
        "renommée : {} / {:?}",
        page.playlist.title, page.playlist.description
    );
    if page.playlist.title != "Seaglass test renommée"
        || page.playlist.description.as_deref() != Some("Nouvelle description")
    {
        return Err("renommage non appliqué".into());
    }
    Ok(())
}
