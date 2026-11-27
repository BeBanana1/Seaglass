//! Library writes on the real API (#4, #81, #201, #204), ignored by default.
//!
//! `cargo test live_collection -- --ignored --nocapture`: **modifies the account**. Only
//! items absent from the favorites are added then removed: no existing favorite is
//! touched, and the cleanup happens even if a check fails.

use super::TidalClient;
use super::live_tests::{ALBUM_ID, client};
use super::models::{LikeTarget, LikedIds, MediaItem, TrackId};

type Check = Result<(), String>;

fn err(e: super::TidalError) -> String {
    e.to_string()
}

/// Like then removal of a track, an album, an artist, another user's playlist and a mix,
/// all absent from the favorites; then a throwaway playlist created, filled, reordered,
/// renamed, emptied and deleted.
#[tokio::test]
#[ignore = "network and TIDAL session required, modifies the library"]
async fn live_collection() {
    let client = client().await;
    let album = client.album(ALBUM_ID).await.unwrap();
    // `favorites/ids` contains all the liked tracks, unavailable ones included (451 against
    // 426 shown on 2026-10-30): the favorites list is a subset of it.
    let liked = client.liked_ids().await.unwrap();
    let favorites = favorite_track_ids(&client).await;
    println!(
        "likes: {} tracks, {} albums, {} artists, {} playlists; {} favorite tracks",
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
    assert_eq!(fresh.len(), 4, "not enough tracks outside the favorites");

    let targets = fresh_targets(&client, &liked).await;
    let album_target = targets[0].clone();
    let mut results = Vec::new();
    for target in [vec![LikeTarget::Track { id: fresh[0] }], targets].concat() {
        // Never remove an existing favorite: an item already present is skipped.
        if contains(&server_ids(&client).await.unwrap(), &target) {
            results.push((target, Err("already in the favorites".to_owned())));
            continue;
        }
        let added = check_added(&client, &target).await;
        // Always remove: the item was not in the favorites before the test.
        let removed = client.set_liked(&target, false).await.map_err(err);
        let absent = check_absent(&client, &target).await;
        println!("{target:?}: add {added:?}, removal {removed:?}, absent {absent:?}");
        results.push((target, added.and(removed).and(absent)));
    }
    // Removing a favorite already removed (elsewhere, for example): idempotent or error?
    if !contains(&server_ids(&client).await.unwrap(), &album_target) {
        let twice = client.set_liked(&album_target, false).await;
        println!("second removal of the album: {twice:?}");
    }

    let uuid = client
        .create_playlist(
            "Seaglass test (à supprimer)",
            "Créée par un test, supprimée aussitôt",
        )
        .await
        .unwrap();
    println!("playlist created: {uuid}");
    let playlist = check_playlist(&client, &uuid, &fresh).await;
    let deleted = client.delete_playlist(&uuid).await;
    let gone = client.playlist(&uuid).await;
    println!(
        "after deletion: {:?}",
        gone.as_ref().map(|p| &p.playlist.title)
    );

    for (target, result) in results {
        assert!(result.is_ok(), "{target:?}: {result:?}");
    }
    playlist.unwrap();
    deleted.unwrap();
    assert!(gone.is_err(), "playlist still readable after deletion");
}

/// An album, an artist, another user's playlist and a mix, all absent from the favorites.
async fn fresh_targets(client: &TidalClient, liked: &LikedIds) -> Vec<LikeTarget> {
    let search = client.search("interstellar", 10).await.unwrap();
    let album = search
        .albums
        .iter()
        .find(|a| !liked.albums.contains(&a.id))
        .expect("no album outside the favorites");
    let artist = search
        .artists
        .iter()
        .find(|a| !liked.artists.contains(&a.id))
        .expect("no artist outside the favorites");
    let own = client.own_playlists().await.unwrap();
    let playlist = search
        .playlists
        .iter()
        .find(|p| !liked.playlists.contains(&p.uuid) && !own.iter().any(|o| o.uuid == p.uuid))
        .expect("no playlist outside the favorites");
    println!(
        "targets: album {} \"{}\", artist {} \"{}\", playlist {} \"{}\"",
        album.id, album.title, artist.id, artist.name, playlist.uuid, playlist.title
    );
    // A home mix absent from the favorites (#204).
    let home = client.home().await.unwrap();
    let mix = home
        .iter()
        .flat_map(|row| &row.items)
        .find_map(|item| match item {
            MediaItem::Mix(m) if !liked.mixes.contains(&m.id) => Some(m.clone()),
            _ => None,
        })
        .expect("no mix outside the favorites");
    println!("mix {} \"{}\"", mix.id, mix.title);
    vec![
        LikeTarget::Album { id: album.id },
        LikeTarget::Artist { id: artist.id },
        LikeTarget::Playlist {
            uuid: playlist.uuid.clone(),
        },
        LikeTarget::Mix { id: mix.id },
    ]
}

/// Read again from the server, without the in-memory set.
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

/// Position of the item in the library list (favorites from most recent to oldest),
/// `None` if it is not there.
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
        return Err("absent from the in-memory favorites".into());
    }
    if !contains(&server_ids(client).await?, target) {
        return Err("absent from favorites/ids".into());
    }
    let position = library_position(client, target).await?;
    println!("added: position {position:?} in the library");
    // Tracks, albums and artists are sorted by added date; playlists and mixes are not.
    let expected = !matches!(target, LikeTarget::Playlist { .. } | LikeTarget::Mix { .. });
    match position {
        Some(0) => Ok(()),
        Some(_) if !expected => Ok(()),
        other => Err(format!("position in the library: {other:?}")),
    }
}

async fn check_absent(client: &TidalClient, target: &LikeTarget) -> Check {
    if contains(&client.liked_ids().await.map_err(err)?, target)
        || contains(&server_ids(client).await?, target)
    {
        return Err("still in the favorites".into());
    }
    match library_position(client, target).await? {
        None => Ok(()),
        Some(i) => Err(format!("still in the library (position {i})")),
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
        return Err("created playlist absent from own_playlists".into());
    }

    let added = client.add_to_playlist(uuid, &ids[..2]).await.map_err(err)?;
    let after_add = tracks(uuid).await?;
    println!("adding {added} tracks: {after_add:?}");
    if added != 2 || after_add != ids[..2] {
        return Err(format!("add: {added}, {after_add:?}"));
    }

    // Duplicate (onDupes=SKIP): ignored without an error, `addedItemIds` empty.
    let dupe = client.add_to_playlist(uuid, &ids[..1]).await;
    let after_dupe = tracks(uuid).await?;
    println!("duplicate: {dupe:?}, tracks {after_dupe:?}");
    if !matches!(dupe, Ok(0)) || after_dupe != ids[..2] {
        return Err(format!("duplicate: {dupe:?}"));
    }

    // Stale ETag: a change made in the meantime must be refused (412).
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
    println!("stale ETag: {:?}", stale_write.as_ref().map(|r| r.status()));
    if !matches!(stale_write, Err(super::TidalError::Conflict(_))) {
        return Err("stale ETag accepted".into());
    }

    let before = tracks(uuid).await?;
    // Wrong track expected at this position: refused without removing anything.
    let wrong = client.remove_from_playlist(uuid, 0, before[1]).await;
    if !matches!(wrong, Err(super::TidalError::Conflict(_))) {
        return Err(format!("removal not verified: {wrong:?}"));
    }
    client
        .remove_from_playlist(uuid, 0, before[0])
        .await
        .map_err(err)?;
    let after = tracks(uuid).await?;
    println!("before removal: {before:?}, after removing index 0: {after:?}");
    if after != before[1..] {
        return Err(format!("removal: {after:?}"));
    }

    // Move (#201): the first track ends up last; the expected track is verified.
    client.add_to_playlist(uuid, &ids[..1]).await.map_err(err)?;
    let before = tracks(uuid).await?;
    let wrong = client.move_in_playlist(uuid, 0, 2, before[1]).await;
    if !matches!(wrong, Err(super::TidalError::Conflict(_))) {
        return Err(format!("move not verified: {wrong:?}"));
    }
    let last = before.len() - 1;
    client
        .move_in_playlist(uuid, 0, last, before[0])
        .await
        .map_err(err)?;
    let moved = tracks(uuid).await?;
    println!("before move: {before:?}, after 0 → {last}: {moved:?}");
    let mut expected = before[1..].to_vec();
    expected.push(before[0]);
    if moved != expected {
        return Err(format!("move: {moved:?}"));
    }
    // Two chained moves (Alt+↓ repeated): the ETag read right after a write is sometimes
    // the old one, `retry_stale` must absorb the 412.
    client
        .move_in_playlist(uuid, 0, 1, moved[0])
        .await
        .map_err(err)?;
    client
        .move_in_playlist(uuid, 1, 2, moved[0])
        .await
        .map_err(err)?;

    // Title and description (#201).
    client
        .edit_playlist(uuid, "Seaglass test renommée", "Nouvelle description")
        .await
        .map_err(err)?;
    let page = client.playlist(uuid).await.map_err(err)?;
    println!(
        "renamed: {} / {:?}",
        page.playlist.title, page.playlist.description
    );
    if page.playlist.title != "Seaglass test renommée"
        || page.playlist.description.as_deref() != Some("Nouvelle description")
    {
        return Err("rename not applied".into());
    }
    Ok(())
}

/// Undoing a removal (#349): `toIndex` on add puts the track back in place, duplicates
/// included (`onDupes=ADD`). Throwaway playlist, no favorite touched.
/// `cargo test live_playlist_insert -- --ignored --nocapture`
#[tokio::test]
#[ignore = "network and TIDAL session required, creates then deletes a playlist"]
async fn live_playlist_insert() {
    let client = client().await;
    let album = client.album(ALBUM_ID).await.unwrap();
    let ids: Vec<TrackId> = album.tracks.iter().map(|t| t.id).take(4).collect();
    assert_eq!(ids.len(), 4);
    let uuid = client
        .create_playlist("Seaglass test (à supprimer)", "Créée par un test")
        .await
        .unwrap();
    let result = check_insert(&client, &uuid, &ids).await;
    let deleted = client.delete_playlist(&uuid).await;
    result.unwrap();
    deleted.unwrap();
}

/// Multiple selection (#332): removal of several positions in one request, undo, then
/// like and removal of several tracks absent from the favorites.
/// `cargo test live_batch -- --ignored --nocapture`
#[tokio::test]
#[ignore = "network and TIDAL session required, creates then deletes a playlist"]
async fn live_batch() {
    let client = client().await;
    let album = client.album(ALBUM_ID).await.unwrap();
    let ids: Vec<TrackId> = album.tracks.iter().map(|t| t.id).take(6).collect();
    assert_eq!(ids.len(), 6);
    let uuid = client
        .create_playlist("Seaglass test (à supprimer)", "Créée par un test")
        .await
        .unwrap();
    let result = check_batch_remove(&client, &uuid, &ids).await;
    let deleted = client.delete_playlist(&uuid).await;

    let liked = server_ids(&client).await.unwrap();
    let fresh: Vec<TrackId> = ids
        .iter()
        .copied()
        .filter(|id| !liked.tracks.contains(id))
        .take(3)
        .collect();
    let likes = check_batch_likes(&client, &fresh).await;
    // Always remove: these tracks were not in the favorites before the test.
    let cleanup = client.set_tracks_liked(&fresh, false).await;

    result.unwrap();
    deleted.unwrap();
    likes.unwrap();
    cleanup.unwrap();
}

async fn check_batch_remove(client: &TidalClient, uuid: &str, ids: &[TrackId]) -> Check {
    let tracks = async || -> Result<Vec<TrackId>, String> {
        let page = client.playlist(uuid).await.map_err(err)?;
        Ok(page.tracks.into_iter().map(|t| t.track.id).collect())
    };
    client.add_to_playlist(uuid, ids).await.map_err(err)?;
    // Wrong track expected at one of the positions: nothing is removed.
    let wrong = client
        .remove_many_from_playlist(uuid, &[(1, ids[1]), (3, ids[2])])
        .await;
    if !matches!(wrong, Err(super::TidalError::Conflict(_))) || tracks().await? != ids {
        return Err(format!("batch removal not verified: {wrong:?}"));
    }
    let picked = [(4, ids[4]), (0, ids[0]), (2, ids[2])];
    client
        .remove_many_from_playlist(uuid, &picked)
        .await
        .map_err(err)?;
    let after = tracks().await?;
    println!("removal of positions 0, 2, 4: {after:?}");
    if after != [ids[1], ids[3], ids[5]] {
        return Err(format!("batch removal: {after:?}"));
    }
    let restored = client
        .restore_many_to_playlist(uuid, &picked)
        .await
        .map_err(err)?;
    let after = tracks().await?;
    println!("restore: {restored} tracks, {after:?}");
    if restored != 3 || after != ids {
        return Err(format!("batch restore: {after:?}"));
    }
    Ok(())
}

async fn check_batch_likes(client: &TidalClient, ids: &[TrackId]) -> Check {
    if ids.len() < 2 {
        return Err("not enough tracks outside the favorites".into());
    }
    client.set_tracks_liked(ids, true).await.map_err(err)?;
    let after = server_ids(client).await?;
    println!(
        "like of {ids:?}: {}",
        ids.iter().all(|id| after.tracks.contains(id))
    );
    if !ids.iter().all(|id| after.tracks.contains(id)) {
        return Err("incomplete batch like".into());
    }
    client.set_tracks_liked(ids, false).await.map_err(err)?;
    let after = server_ids(client).await?;
    if ids.iter().any(|id| after.tracks.contains(id)) {
        return Err("incomplete batch removal from the favorites".into());
    }
    Ok(())
}

async fn check_insert(client: &TidalClient, uuid: &str, ids: &[TrackId]) -> Check {
    let tracks = async || -> Result<Vec<TrackId>, String> {
        let page = client.playlist(uuid).await.map_err(err)?;
        Ok(page.tracks.into_iter().map(|t| t.track.id).collect())
    };
    client.add_to_playlist(uuid, ids).await.map_err(err)?;
    for index in [1, 0, 3] {
        client
            .remove_from_playlist(uuid, index, ids[index])
            .await
            .map_err(err)?;
        let added = client
            .insert_in_playlist(uuid, index, ids[index])
            .await
            .map_err(err)?;
        let after = tracks().await?;
        println!("removal then restore at index {index}: added {added}, {after:?}");
        if !added || after != ids {
            return Err(format!("restore at index {index}: {after:?}"));
        }
    }
    // Duplicate: the first track put back at index 2 as well.
    let added = client
        .insert_in_playlist(uuid, 2, ids[0])
        .await
        .map_err(err)?;
    let after = tracks().await?;
    println!("duplicate at index 2: added {added}, {after:?}");
    let expected = [ids[0], ids[1], ids[0], ids[2], ids[3]];
    if !added || after != expected {
        return Err(format!("duplicate: {after:?}"));
    }
    Ok(())
}
