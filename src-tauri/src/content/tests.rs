use super::{ContentFilter, Filtered};
use crate::tidal::models::{
    AlbumPage, AlbumRef, AlbumSummary, ArtistRef, HomeRow, MediaItem, PlaylistPage,
    PlaylistSummary, SearchResults, Track, TrackEntry, TrackId,
};

const HIDE_EXPLICIT: ContentFilter = ContentFilter {
    hide_explicit: true,
};

fn track(id: u64, explicit: bool) -> Track {
    Track {
        id: TrackId(id),
        title: format!("Titre {id}"),
        version: None,
        duration: 200,
        artists: vec![ArtistRef {
            id: 1,
            name: "A".into(),
        }],
        album: AlbumRef {
            id: 9,
            title: "Album".into(),
            cover_url: None,
            cover_small_url: None,
            cover_large_url: None,
            color: None,
        },
        explicit,
        hires: false,
        isrc: None,
    }
}

fn album(id: u64, explicit: bool) -> AlbumSummary {
    AlbumSummary {
        id,
        title: format!("Album {id}"),
        artists: Vec::new(),
        cover_url: None,
        cover_large_url: None,
        color: None,
        release_date: None,
        number_of_tracks: None,
        album_type: None,
        explicit,
        hires: false,
    }
}

fn ids(tracks: &[Track]) -> Vec<u64> {
    tracks.iter().map(|t| t.id.0).collect()
}

fn playlist(mine: bool) -> PlaylistPage {
    PlaylistPage {
        playlist: PlaylistSummary {
            uuid: "4ba30155-d807-446f-9e53-b67520ccfe83".into(),
            title: "P".into(),
            description: None,
            image_url: None,
            number_of_tracks: Some(2),
            duration: None,
            editorial: false,
            mine,
        },
        tracks: [track(1, true), track(2, false)]
            .into_iter()
            .map(|track| TrackEntry {
                track,
                added_at: None,
            })
            .collect(),
        revision: None,
    }
}

#[test]
fn default_filter_hides_nothing() {
    let mut tracks = vec![track(1, true), track(2, false)];
    tracks.filter_with(&ContentFilter::default());
    assert_eq!(ids(&tracks), [1, 2]);
}

#[test]
fn explicit_tracks_and_albums_leave_search_results() {
    let mut results = SearchResults {
        tracks: vec![track(1, true), track(2, false)],
        albums: vec![album(10, false), album(11, true)],
        ..SearchResults::default()
    };
    results.filter_with(&HIDE_EXPLICIT);
    assert_eq!(ids(&results.tracks), [2]);
    assert_eq!(
        results.albums.iter().map(|a| a.id).collect::<Vec<_>>(),
        [10]
    );
}

#[test]
fn album_page_keeps_the_album_but_not_its_hidden_tracks() {
    let mut page = AlbumPage {
        album: album(10, true),
        cover_large_url: None,
        duration: None,
        copyright: None,
        tracks: vec![track(1, true), track(2, false)],
    };
    page.filter_with(&HIDE_EXPLICIT);
    assert_eq!(page.album.id, 10);
    assert_eq!(ids(&page.tracks), [2]);
}

#[test]
fn own_playlists_stay_complete() {
    let mut mine = playlist(true);
    mine.filter_with(&HIDE_EXPLICIT);
    assert_eq!(mine.tracks.len(), 2);
    let mut theirs = Some(playlist(false));
    theirs.filter_with(&HIDE_EXPLICIT);
    assert_eq!(theirs.map(|p| p.tracks.len()), Some(1));
}

#[test]
fn home_rows_emptied_by_the_filter_disappear() {
    let row = |key: &str, items: Vec<MediaItem>| HomeRow {
        key: key.into(),
        title: key.into(),
        items,
    };
    let mut rows = vec![
        row("explicit", vec![MediaItem::Album(album(1, true))]),
        row(
            "mixed",
            vec![
                MediaItem::Album(album(2, true)),
                MediaItem::Album(album(3, false)),
            ],
        ),
        row("empty", Vec::new()),
    ];
    rows.filter_with(&HIDE_EXPLICIT);
    let keys: Vec<&str> = rows.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys, ["mixed", "empty"]);
    assert_eq!(rows[0].items.len(), 1);
}

#[test]
fn hidden_twin_album_becomes_none() {
    let mut twin = Some(album(1, true));
    twin.filter_with(&HIDE_EXPLICIT);
    assert_eq!(twin, None);
}

#[test]
fn settings_without_content_field_read_as_default() {
    let filter: ContentFilter = serde_json::from_str("{}").unwrap();
    assert_eq!(filter, ContentFilter::default());
    let filter: ContentFilter = serde_json::from_str(r#"{"hideExplicit":true}"#).unwrap();
    assert!(filter.hide_explicit);
}
