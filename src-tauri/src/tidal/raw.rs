//! Formats JSON bruts de l'API (v1 et flux d'accueil v2) et leur conversion en `models`.
//! Rien de ce fichier ne sort du module `tidal`.

use serde::Deserialize;

use super::date::timestamp_ms;
use super::models::{
    AlbumRef, AlbumSummary, ArtistRef, ArtistSummary, PlaylistSummary, Track, TrackEntry, TrackId,
};

const IMAGE_URL: &str = "https://resources.tidal.com/images";

/// URL d'image à partir d'un identifiant de ressource (`a-b-c` → `a/b/c/<taille>.jpg`).
pub(super) fn image_url(id: &str, size: u32) -> String {
    format!("{IMAGE_URL}/{}/{size}x{size}.jpg", id.replace('-', "/"))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawPage<T> {
    pub items: Vec<T>,
    pub total_number_of_items: Option<u32>,
}

/// Entrée des favoris : `{ "created": …, "item": … }` (`created` = date du like).
#[derive(Deserialize)]
pub(super) struct RawFavorite<T> {
    pub created: Option<String>,
    pub item: T,
}

/// Entrée de `playlistsAndFavoritePlaylists` : `{ "type": …, "playlist": … }`.
#[derive(Deserialize)]
pub(super) struct RawPlaylistEntry {
    pub playlist: RawPlaylist,
}

#[derive(Deserialize)]
pub(super) struct RawSearch {
    #[serde(default = "RawPage::empty")]
    pub tracks: RawPage<RawTrack>,
    #[serde(default = "RawPage::empty")]
    pub albums: RawPage<RawAlbumFull>,
    #[serde(default = "RawPage::empty")]
    pub artists: RawPage<RawArtistFull>,
    #[serde(default = "RawPage::empty")]
    pub playlists: RawPage<RawPlaylist>,
}

impl<T> RawPage<T> {
    fn empty() -> Self {
        Self {
            items: Vec::new(),
            total_number_of_items: Some(0),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawTrack {
    id: u64,
    title: String,
    version: Option<String>,
    duration: u32,
    #[serde(default)]
    explicit: bool,
    #[serde(default)]
    artists: Vec<RawArtist>,
    /// Absent dans certains flux (titres de l'accueil) : ces titres sont ignorés.
    album: Option<RawAlbum>,
    media_metadata: Option<RawMediaMetadata>,
    /// Date d'ajout, présente seulement dans `playlists/{uuid}/tracks` (vérifié).
    date_added: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawArtist {
    id: u64,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAlbum {
    id: u64,
    title: String,
    cover: Option<String>,
    vibrant_color: Option<String>,
}

#[derive(Deserialize)]
struct RawMediaMetadata {
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawAlbumFull {
    pub id: u64,
    pub title: String,
    pub cover: Option<String>,
    vibrant_color: Option<String>,
    #[serde(default)]
    pub artists: Vec<RawArtist>,
    pub release_date: Option<String>,
    pub number_of_tracks: Option<u32>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub explicit: bool,
    media_metadata: Option<RawMediaMetadata>,
    pub duration: Option<u32>,
    pub copyright: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawArtistFull {
    pub id: u64,
    pub name: String,
    pub picture: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawPlaylist {
    pub uuid: String,
    pub title: String,
    pub description: Option<String>,
    pub square_image: Option<String>,
    pub number_of_tracks: Option<u32>,
    pub duration: Option<u32>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub creator: Option<RawCreator>,
}

#[derive(Deserialize)]
pub(super) struct RawCreator {
    pub id: Option<u64>,
}

/// Ligne du flux d'accueil v2 : `{ "type", "moduleId", "title", "items": [{ "type", "data" }] }`.
#[derive(Deserialize)]
pub(super) struct RawHomeRow {
    /// Identifiant stable du module (`YOUR_FAVORITE_ARTISTS`, `BECAUSE_YOU_LISTENED_TO_ALBUM`…),
    /// indépendant de la langue et du contenu de la ligne.
    #[serde(rename = "moduleId")]
    pub module_id: Option<String>,
    pub title: Option<String>,
    #[serde(default)]
    pub items: Vec<RawHomeItem>,
}

#[derive(Deserialize)]
pub(super) struct RawHomeItem {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

#[derive(Deserialize)]
pub(super) struct RawHome {
    #[serde(default)]
    pub items: Vec<RawHomeRow>,
}

/// Garde uniquement une couleur `#rrggbb` : la valeur part telle quelle dans le CSS.
fn hex_color(color: Option<String>) -> Option<String> {
    color.filter(|c| {
        c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|ch| ch.is_ascii_hexdigit())
    })
}

fn has_hires(meta: Option<&RawMediaMetadata>) -> bool {
    meta.is_some_and(|m| m.tags.iter().any(|t| t == "HIRES_LOSSLESS"))
}

fn artist_refs(artists: Vec<RawArtist>) -> Vec<ArtistRef> {
    artists
        .into_iter()
        .map(|a| ArtistRef {
            id: a.id,
            name: a.name,
        })
        .collect()
}

impl RawTrack {
    pub(super) fn into_track(self) -> Option<Track> {
        let album = self.album?;
        Some(Track {
            id: TrackId(self.id),
            title: self.title,
            version: self.version,
            duration: self.duration,
            artists: artist_refs(self.artists),
            album: AlbumRef {
                id: album.id,
                title: album.title,
                cover_url: album.cover.as_deref().map(|c| image_url(c, 320)),
                color: hex_color(album.vibrant_color),
            },
            explicit: self.explicit,
            hires: has_hires(self.media_metadata.as_ref()),
        })
    }
}

pub(super) fn tracks(raw: Vec<RawTrack>) -> Vec<Track> {
    raw.into_iter().filter_map(RawTrack::into_track).collect()
}

/// Titres de playlist, avec leur `dateAdded`.
pub(super) fn playlist_entries(raw: Vec<RawTrack>) -> Vec<TrackEntry> {
    raw.into_iter()
        .filter_map(|t| {
            let added_at = t.date_added.as_deref().and_then(timestamp_ms);
            t.into_track().map(|track| TrackEntry { track, added_at })
        })
        .collect()
}

/// Titres favoris, avec la date du like (`created`).
pub(super) fn favorite_entries(raw: Vec<RawFavorite<RawTrack>>) -> Vec<TrackEntry> {
    raw.into_iter()
        .filter_map(|f| {
            let added_at = f.created.as_deref().and_then(timestamp_ms);
            f.item
                .into_track()
                .map(|track| TrackEntry { track, added_at })
        })
        .collect()
}

impl From<RawAlbumFull> for AlbumSummary {
    fn from(raw: RawAlbumFull) -> Self {
        AlbumSummary {
            id: raw.id,
            title: raw.title,
            artists: artist_refs(raw.artists),
            cover_url: raw.cover.as_deref().map(|c| image_url(c, 320)),
            color: hex_color(raw.vibrant_color),
            release_date: raw.release_date,
            number_of_tracks: raw.number_of_tracks,
            album_type: raw.kind,
            explicit: raw.explicit,
            hires: has_hires(raw.media_metadata.as_ref()),
        }
    }
}

impl From<RawArtistFull> for ArtistSummary {
    fn from(raw: RawArtistFull) -> Self {
        ArtistSummary {
            id: raw.id,
            name: raw.name,
            picture_url: raw.picture.as_deref().map(|p| image_url(p, 320)),
        }
    }
}

impl RawPlaylist {
    /// `user_id` : utilisateur connecté, pour signaler ses propres playlists.
    pub(super) fn into_summary(self, user_id: Option<u64>) -> PlaylistSummary {
        let creator = self.creator.and_then(|c| c.id);
        PlaylistSummary {
            uuid: self.uuid,
            title: self.title,
            description: self.description.filter(|d| !d.trim().is_empty()),
            image_url: self.square_image.as_deref().map(|i| image_url(i, 320)),
            number_of_tracks: self.number_of_tracks,
            duration: self.duration,
            editorial: self.kind.as_deref() == Some("EDITORIAL"),
            mine: creator.is_some() && creator == user_id,
        }
    }
}

/// Réponse de `tracks/{id}/lyrics` (`subtitles` au format LRC, `lyrics` en texte simple).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawLyrics {
    pub lyrics_provider: Option<String>,
    #[serde(default)]
    pub is_right_to_left: bool,
    pub lyrics: Option<String>,
    pub subtitles: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_url_from_resource_id() {
        assert_eq!(
            image_url("3f5f9e03-c58c-4053", 320),
            "https://resources.tidal.com/images/3f5f9e03/c58c/4053/320x320.jpg"
        );
    }

    #[test]
    fn only_hex_colors_are_kept() {
        assert_eq!(
            hex_color(Some("#4f95b0".into())).as_deref(),
            Some("#4f95b0")
        );
        assert_eq!(hex_color(Some("red;}body{".into())), None);
        assert_eq!(hex_color(Some("#4f95b".into())), None);
        assert_eq!(hex_color(None), None);
    }

    #[test]
    fn track_without_album_is_skipped() {
        let raw: Vec<RawTrack> = serde_json::from_str(
            r#"[{"id":1,"title":"a","duration":10,"artists":[]},
                {"id":2,"title":"b","duration":10,"artists":[],"album":{"id":3,"title":"c","cover":null}}]"#,
        )
        .unwrap();
        let t = tracks(raw);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].id, TrackId(2));
    }

    #[test]
    fn entries_keep_their_date() {
        let raw: Vec<RawTrack> = serde_json::from_str(
            r#"[{"id":1,"title":"a","duration":10,"album":{"id":3,"title":"c"},
                 "dateAdded":"1970-01-01T00:00:01.000+0000"},
                {"id":2,"title":"b","duration":10,"album":{"id":3,"title":"c"},"dateAdded":"?"}]"#,
        )
        .unwrap();
        let e = playlist_entries(raw);
        assert_eq!(e[0].added_at, Some(1000));
        assert_eq!(e[1].added_at, None);

        let fav: Vec<RawFavorite<RawTrack>> = serde_json::from_str(
            r#"[{"created":"1970-01-01T00:00:02.000+0000",
                 "item":{"id":1,"title":"a","duration":10,"album":{"id":3,"title":"c"}}}]"#,
        )
        .unwrap();
        assert_eq!(favorite_entries(fav)[0].added_at, Some(2000));
    }

    /// `TrackEntry` est sérialisé à plat : l'UI le lit comme un `Track`.
    #[test]
    fn entry_is_flat() {
        let raw: Vec<RawTrack> = serde_json::from_str(
            r#"[{"id":1,"title":"a","duration":10,"album":{"id":3,"title":"c"},
                 "dateAdded":"1970-01-01T00:00:01.000+0000"}]"#,
        )
        .unwrap();
        let value = serde_json::to_value(&playlist_entries(raw)[0]).unwrap();
        assert_eq!(value["id"], 1);
        assert_eq!(value["album"]["title"], "c");
        assert_eq!(value["addedAt"], 1000);
    }

    #[test]
    fn playlist_ownership() {
        let raw: RawPlaylist = serde_json::from_str(
            r#"{"uuid":"u","title":"t","description":"  ","squareImage":null,"type":"USER","creator":{"id":42}}"#,
        )
        .unwrap();
        let p = raw.into_summary(Some(42));
        assert!(p.mine);
        assert!(!p.editorial);
        assert_eq!(p.description, None);
    }
}
