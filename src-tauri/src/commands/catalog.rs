//! Commandes de navigation : recherche, pages album / artiste / playlist / mix, radios,
//! bibliothèque, accueil, fil d'actualité, paroles, infos d'un titre.

use serde::Serialize;
use tauri::{AppHandle, State};

use super::settings::home_layout::HomeSection;
use super::snapshots::{self, LibraryTab};
use super::{AppState, CmdResult, err, settings};
use crate::i18n::{tr, trf};
use crate::player::{HiresUpgrade, PlaybackSettings, is_better};
use crate::tidal::models::{
    AlbumCreditGroup, AlbumPage, AlbumSummary, ArtistExtra, ArtistPage, ArtistSummary, FeedItem,
    HomeRow, LibraryList, Lyrics, MixPage, MixSummary, PlaylistPage, PlaylistSummary, RadioPage,
    RadioSeed, SearchResults, TrackEntry, TrackId, TrackInfo,
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
    // L'historique n'est pas tenu ici : l'UI cherche pendant la frappe (#248) et
    // n'enregistre que la requête retenue (`note_search_history`).
    state.tidal.search(query, SEARCH_LIMIT).await.map_err(err)
}

#[tauri::command]
pub async fn album(state: State<'_, AppState>, id: u64) -> CmdResult<AlbumPage> {
    state.tidal.album(id).await.map_err(err)
}

#[tauri::command]
pub async fn artist(state: State<'_, AppState>, id: u64) -> CmdResult<ArtistPage> {
    state.tidal.artist(id).await.map_err(err)
}

/// Page à jour de la playlist, ou `null` si elle n'a pas changé depuis `revision` (la
/// `revision` de la page déjà affichée, #152).
#[tauri::command]
pub async fn playlist(
    state: State<'_, AppState>,
    uuid: String,
    revision: Option<String>,
) -> CmdResult<Option<PlaylistPage>> {
    check_playlist_uuid(&uuid)?;
    // Un ETag de TIDAL fait quelques dizaines de caractères ; il n'est que comparé.
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
    Ok(page)
}

pub(super) fn check_playlist_uuid(uuid: &str) -> CmdResult<()> {
    if is_uuid(uuid) {
        Ok(())
    } else {
        Err(tr("identifiant de playlist invalide", "invalid playlist ID").into())
    }
}

/// Biographie, artistes similaires et « Apparaît sur » (#205), après la page elle-même.
#[tauri::command]
pub async fn artist_extra(state: State<'_, AppState>, id: u64) -> CmdResult<ArtistExtra> {
    Ok(state.tidal.artist_extra(id).await)
}

#[tauri::command]
pub async fn mix(state: State<'_, AppState>, id: String) -> CmdResult<MixPage> {
    if !is_mix_id(&id) {
        return Err(tr("identifiant de mix invalide", "invalid mix ID").into());
    }
    state.tidal.mix(&id).await.map_err(err)
}

#[tauri::command]
pub async fn radio(state: State<'_, AppState>, seed: RadioSeed, id: u64) -> CmdResult<RadioPage> {
    state.tidal.radio(seed, id).await.map_err(err)
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
    // La liste complète met à jour les cœurs (likes faits sur un autre appareil).
    super::collection::emit_likes(&app, &state).await;
    Ok(tracks)
}

// Comme pour les titres, chaque liste complète met à jour les favoris affichés (#81).
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

/// Première page de l'accueil, déjà disposée.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeFeed {
    pub sections: Vec<HomeSection>,
    /// D'autres pages existent : l'UI appelle ensuite `home_rest` (#207).
    pub more: bool,
}

/// Lignes de la première page de l'accueil dans l'ordre choisi, masquées comprises
/// (marquées `hidden`). La suite du flux vient de `home_rest`, pour que l'accueil
/// s'affiche sans attendre ses pages suivantes (~0,5 s chacune).
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

/// Tout l'accueil (première page et suivantes), disposé : remplace la première page.
#[tauri::command]
pub async fn home_rest(state: State<'_, AppState>) -> CmdResult<Vec<HomeSection>> {
    let rows = state.tidal.home_rest().await.map_err(err)?;
    Ok(settings::load(&state)
        .home
        .arrange(with_local_row(&state, rows)))
}

/// « Écoutés dans Seaglass » (#202) en tête du flux : la disposition enregistrée s'applique
/// ensuite comme aux lignes de TIDAL.
fn with_local_row(state: &AppState, mut rows: Vec<HomeRow>) -> Vec<HomeRow> {
    if let Some(row) = super::play_history::home_row(state) {
        rows.insert(0, row);
    }
    rows
}

/// Paroles d'un titre ; `None` si TIDAL n'en a pas (ce n'est pas une erreur).
#[tauri::command]
pub async fn lyrics(state: State<'_, AppState>, track_id: TrackId) -> CmdResult<Option<Lyrics>> {
    state.tidal.lyrics(track_id).await.map_err(err)
}

/// Infos détaillées d'un titre (#3), à la demande (une requête de lecture). `played` dit
/// si Seaglass lirait l'édition hi-res trouvée à la place du titre sans demander (réglage
/// automatique ; en mode « Demander », c'est la barre de lecture qui la propose, #59).
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

/// Crédits de l'album entier (#206), à la demande.
#[tauri::command]
pub async fn album_credits(
    state: State<'_, AppState>,
    album_id: u64,
) -> CmdResult<Vec<AlbumCreditGroup>> {
    state.tidal.album_credits(album_id).await.map_err(err)
}

/// Album hi-res au même titre de l'artiste principal, pour un album qui ne l'est pas (#59).
#[tauri::command]
pub async fn hires_twin_album(
    state: State<'_, AppState>,
    album_id: u64,
) -> CmdResult<Option<AlbumSummary>> {
    state.tidal.hires_twin_album(album_id).await.map_err(err)
}

/// Sorties récentes des artistes suivis.
#[tauri::command]
pub async fn feed(state: State<'_, AppState>) -> CmdResult<Vec<FeedItem>> {
    state.tidal.feed().await.map_err(err)
}

/// UUID au format `8-4-4-4-12` (hexadécimal), seul format de playlist accepté dans l'URL.
pub(super) fn is_uuid(s: &str) -> bool {
    let groups: Vec<&str> = s.split('-').collect();
    groups.len() == 5
        && groups
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(g, len)| g.len() == len && g.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Identifiant de mix : alphanumérique (30 caractères hexadécimaux observés), il part dans l'URL.
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
