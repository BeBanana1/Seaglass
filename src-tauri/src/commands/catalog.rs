//! Navigation commands: search, album / artist / playlist / mix pages, radios,
//! library, home, lyrics, track info. The news feed is in `feed.rs`.

use serde::Serialize;
use tauri::{AppHandle, State};

use super::settings::home_layout::HomeSection;
use super::snapshots::{self, LibraryTab};
use super::{AppState, CmdResult, err, settings};
use crate::content::filtered;
use crate::i18n::{tr, trf};
use crate::player::{HiresUpgrade, PlaybackSettings, is_better};
use crate::tidal::models::{
    AlbumCreditGroup, AlbumPage, AlbumRelated, AlbumSummary, ArtistExtra, ArtistPage,
    ArtistSummary, ContributorPage, HomeRow, LibraryList, Lyrics, MixPage, MixSummary,
    PlaylistPage, PlaylistSummary, RadioPage, RadioSeed, SearchResults, TrackEntry, TrackId,
    TrackInfo,
};

pub(super) const MAX_QUERY_LEN: usize = 200;
const SEARCH_LIMIT: u32 = 50;

#[tauri::command]
pub async fn search(state: State<'_, AppState>, query: String) -> CmdResult<SearchResults> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(SearchResults::default());
    }
    if query.chars().count() > MAX_QUERY_LEN {
        return Err(trf!(
            "recherche limitée à {MAX_QUERY_LEN} caractères",
            "search limited to {MAX_QUERY_LEN} characters"
        ));
    }
    // History is not kept here: the UI searches while typing (#248) and
    // only records the query that is kept (`note_search_history`).
    state
        .tidal
        .search(query, SEARCH_LIMIT)
        .await
        .map(filtered)
        .map_err(err)
}

#[tauri::command]
pub async fn album(state: State<'_, AppState>, id: u64) -> CmdResult<AlbumPage> {
    state.tidal.album(id).await.map(filtered).map_err(err)
}

/// Other versions of the album and other albums by the artist (#350), after the page itself.
#[tauri::command]
pub async fn album_related(state: State<'_, AppState>, id: u64) -> CmdResult<AlbumRelated> {
    Ok(filtered(state.tidal.album_related(id).await))
}

#[tauri::command]
pub async fn artist(state: State<'_, AppState>, id: u64) -> CmdResult<ArtistPage> {
    state.tidal.artist(id).await.map(filtered).map_err(err)
}

/// Up-to-date playlist page, or `null` if it has not changed since `revision` (the
/// `revision` of the page already displayed, #152).
#[tauri::command]
pub async fn playlist(
    state: State<'_, AppState>,
    uuid: String,
    revision: Option<String>,
) -> CmdResult<Option<PlaylistPage>> {
    check_playlist_uuid(&uuid)?;
    // A TIDAL ETag is a few dozen characters long; it is only compared.
    if revision.as_ref().is_some_and(|r| r.len() > 256) {
        return Err(tr("révision de playlist invalide", "invalid playlist revision").into());
    }
    let known = revision.as_deref();
    let page = state
        .tidal
        .playlist_since(&uuid, known)
        .await
        .map_err(err)?;
    if let Some(page) = &page {
        snapshots::save_playlist(&state, page);
    }
    Ok(filtered(page))
}

pub(super) fn check_playlist_uuid(uuid: &str) -> CmdResult<()> {
    if is_uuid(uuid) {
        Ok(())
    } else {
        Err(tr("identifiant de playlist invalide", "invalid playlist ID").into())
    }
}

/// Biography, similar artists and "Appears on" (#205), after the page itself.
#[tauri::command]
pub async fn artist_extra(state: State<'_, AppState>, id: u64) -> CmdResult<ArtistExtra> {
    Ok(filtered(state.tidal.artist_extra(id).await))
}

/// Tracks where a person is credited (#429), in batches: `offset` = `nextOffset` of the
/// previous batch, 0 for the page.
#[tauri::command]
pub async fn contributor(
    state: State<'_, AppState>,
    artist_id: u64,
    offset: u32,
) -> CmdResult<ContributorPage> {
    // Well beyond the largest catalogs measured (7,605 tracks for Hans Zimmer).
    if offset > 1_000_000 {
        return Err(tr("position invalide", "invalid offset").into());
    }
    state
        .tidal
        .contributor(artist_id, offset)
        .await
        .map(filtered)
        .map_err(err)
}

#[tauri::command]
pub async fn mix(state: State<'_, AppState>, id: String) -> CmdResult<MixPage> {
    if !is_mix_id(&id) {
        return Err(tr("identifiant de mix invalide", "invalid mix ID").into());
    }
    state.tidal.mix(&id).await.map(filtered).map_err(err)
}

#[tauri::command]
pub async fn radio(state: State<'_, AppState>, seed: RadioSeed, id: u64) -> CmdResult<RadioPage> {
    state.tidal.radio(seed, id).await.map(filtered).map_err(err)
}

#[tauri::command]
pub async fn library_mixes(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<Vec<MixSummary>> {
    let mixes = state.tidal.favorite_mixes().await.map_err(err)?;
    super::collection::emit_likes(&app, &state).await;
    Ok(mixes)
}

#[tauri::command]
pub async fn library_tracks(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<LibraryList<TrackEntry>> {
    let tracks = state.tidal.favorite_tracks().await.map_err(err)?;
    snapshots::save_library(&state, LibraryTab::Tracks, &tracks);
    // The full list updates the hearts (likes made on another device).
    super::collection::emit_likes(&app, &state).await;
    Ok(tracks)
}

// As for tracks, each full list updates the displayed favorites (#81).
#[tauri::command]
pub async fn library_albums(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<LibraryList<AlbumSummary>> {
    let albums = state.tidal.favorite_albums().await.map_err(err)?;
    snapshots::save_library(&state, LibraryTab::Albums, &albums);
    super::collection::emit_likes(&app, &state).await;
    Ok(albums)
}

#[tauri::command]
pub async fn library_artists(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<LibraryList<ArtistSummary>> {
    let artists = state.tidal.favorite_artists().await.map_err(err)?;
    snapshots::save_library(&state, LibraryTab::Artists, &artists);
    super::collection::emit_likes(&app, &state).await;
    Ok(artists)
}

#[tauri::command]
pub async fn library_playlists(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<LibraryList<PlaylistSummary>> {
    let playlists = state.tidal.library_playlists().await.map_err(err)?;
    snapshots::save_library(&state, LibraryTab::Playlists, &playlists);
    super::collection::emit_likes(&app, &state).await;
    Ok(playlists)
}

/// First page of the home, already laid out.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeFeed {
    pub sections: Vec<HomeSection>,
    /// More pages exist: the UI then calls `home_rest` (#207).
    pub more: bool,
}

/// Rows of the home's first page in the chosen order, hidden ones included
/// (marked `hidden`). The rest of the feed comes from `home_rest`, so that the home
/// is displayed without waiting for its following pages (~0.5 s each).
#[tauri::command]
pub async fn home(state: State<'_, AppState>) -> CmdResult<HomeFeed> {
    let start = state.tidal.home_start().await.map_err(err)?;
    Ok(HomeFeed {
        sections: settings::load(&state)
            .home
            .arrange(with_local_row(&state, start.rows)),
        more: start.more,
    })
}

/// The whole home (first and following pages), laid out: replaces the first page.
#[tauri::command]
pub async fn home_rest(state: State<'_, AppState>) -> CmdResult<Vec<HomeSection>> {
    let rows = state.tidal.home_rest().await.map_err(err)?;
    Ok(settings::load(&state)
        .home
        .arrange(with_local_row(&state, rows)))
}

/// "Listened in Seaglass" (#202) at the head of the feed: the saved layout then applies
/// to it as to TIDAL's rows.
/// The content filter (#368) applies to both.
fn with_local_row(state: &AppState, mut rows: Vec<HomeRow>) -> Vec<HomeRow> {
    if let Some(row) = super::play_history::home_row(state) {
        rows.insert(0, row);
    }
    filtered(rows)
}

/// Lyrics of a track; `None` if TIDAL has none (this is not an error).
#[tauri::command]
pub async fn lyrics(state: State<'_, AppState>, track_id: TrackId) -> CmdResult<Option<Lyrics>> {
    state.tidal.lyrics(track_id).await.map_err(err)
}

/// Detailed info of a track (#3), on demand (one read request). `played` says whether
/// Seaglass would play the hi-res edition found instead of the track without asking
/// (automatic setting; in "Ask" mode, the playback bar offers it, #59).
#[tauri::command]
pub async fn track_info(state: State<'_, AppState>, track_id: TrackId) -> CmdResult<TrackInfo> {
    let mut info = state.tidal.track_info(track_id).await.map_err(err)?;
    let upgrade = PlaybackSettings::load(state.store.as_deref()).hires_upgrade;
    if let Some(edition) = info.hires_edition.as_mut() {
        edition.played = upgrade == HiresUpgrade::Auto
            && edition
                .stream
                .as_ref()
                .is_some_and(|s| is_better(s, info.stream.as_ref()));
    }
    Ok(info)
}

/// Credits of the whole album (#206), on demand.
#[tauri::command]
pub async fn album_credits(
    state: State<'_, AppState>,
    album_id: u64,
) -> CmdResult<Vec<AlbumCreditGroup>> {
    state.tidal.album_credits(album_id).await.map_err(err)
}

/// Hi-res album with the same title by the main artist, for an album that is not one (#59).
#[tauri::command]
pub async fn hires_twin_album(
    state: State<'_, AppState>,
    album_id: u64,
) -> CmdResult<Option<AlbumSummary>> {
    state
        .tidal
        .hires_twin_album(album_id)
        .await
        .map(filtered)
        .map_err(err)
}

/// UUID in the `8-4-4-4-12` format (hexadecimal), the only playlist format accepted in the URL.
pub(super) fn is_uuid(s: &str) -> bool {
    let groups: Vec<&str> = s.split('-').collect();
    groups.len() == 5
        && groups
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(g, len)| g.len() == len && g.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Mix identifier: alphanumeric (30 hexadecimal characters observed), it goes in the URL.
pub(super) fn is_mix_id(s: &str) -> bool {
    (1..=64).contains(&s.len()) && s.chars().all(|c| c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::{is_mix_id, is_uuid};

    #[test]
    fn mix_id_validation() {
        assert!(is_mix_id("01636e04b6e9ec28485f7d0372b7f3"));
        assert!(!is_mix_id(""));
        assert!(!is_mix_id("../users/1"));
        assert!(!is_mix_id("abc?x=1"));
        assert!(!is_mix_id(&"a".repeat(65)));
    }

    #[test]
    fn uuid_validation() {
        assert!(is_uuid("4ba30155-d807-446f-9e53-b67520ccfe83"));
        assert!(!is_uuid("4ba30155-d807-446f-9e53"));
        assert!(!is_uuid("../../users/1-d807-446f-9e53-b67520ccfe83"));
        assert!(!is_uuid("4ba30155-d807-446f-9e53-b67520ccfe8z"));
    }
}
