//! Ce que la page des invités reçoit (#25) : titres, albums et état, rien d'autre ne sort
//! de l'app.

use serde::Serialize;

use super::api::{MAX_QUEUE, Snapshot};
use super::guests::Guests;
use super::rules::JamRules;
use crate::tidal::models::{AlbumSummary, ArtistRef, Track};

fn artist_names(artists: &[ArtistRef]) -> String {
    artists
        .iter()
        .map(|a| a.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Titre tel que la page l'affiche.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestTrack {
    pub id: u64,
    pub title: String,
    pub artists: String,
    pub album: String,
    pub cover: Option<String>,
    pub duration: u32,
    /// Invité qui l'a ajouté à la file manuelle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
    /// Lot de la Jam qui l'a ajouté (#223) : la page de son auteur propose de le retirer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch: Option<u32>,
}

impl From<&Track> for GuestTrack {
    fn from(t: &Track) -> Self {
        let title = match t.version.as_deref().filter(|v| !v.is_empty()) {
            Some(v) => format!("{} ({v})", t.title),
            None => t.title.clone(),
        };
        Self {
            id: t.id.0,
            title,
            artists: artist_names(&t.artists),
            album: t.album.title.clone(),
            cover: t.album.cover_small_url.clone(),
            duration: t.duration,
            by: None,
            batch: None,
        }
    }
}

/// Album d'une recherche de la Jam (#223).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestAlbum {
    pub id: u64,
    pub title: String,
    pub artists: String,
    pub cover: Option<String>,
    pub tracks: Option<u32>,
    pub year: Option<String>,
}

impl From<&AlbumSummary> for GuestAlbum {
    fn from(a: &AlbumSummary) -> Self {
        Self {
            id: a.id,
            title: a.title.clone(),
            artists: artist_names(&a.artists),
            cover: a.cover_url.clone(),
            tracks: a.number_of_tracks,
            year: a
                .release_date
                .as_deref()
                .and_then(|d| d.get(..4))
                .map(str::to_owned),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestState {
    pub now: Option<GuestTrack>,
    pub paused: bool,
    /// Les invités peuvent mettre en pause et relancer (#222, choix de l'hôte).
    pub controls: bool,
    /// Les invités peuvent ajouter (l'hôte peut suspendre les ajouts, #288).
    pub adds: bool,
    /// Les invités peuvent ajouter un album entier (#288).
    pub albums: bool,
    /// Titres en attente au plus par invité, si l'hôte en a fixé une (#288).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// File manuelle (avec le nom de l'invité) puis suite du contexte, `MAX_QUEUE` au plus.
    pub queue: Vec<GuestTrack>,
}

impl GuestState {
    pub fn new(snapshot: &Snapshot, guests: &Guests, rules: &JamRules, adds: bool) -> Self {
        let manual = snapshot.manual.iter().map(|t| GuestTrack {
            by: guests.name(t.id.0).map(str::to_owned),
            batch: guests.batch(t.id.0),
            ..GuestTrack::from(t)
        });
        Self {
            now: snapshot.now.as_ref().map(GuestTrack::from),
            paused: snapshot.paused,
            controls: rules.guest_controls,
            adds,
            albums: rules.albums,
            limit: rules.max_pending,
            queue: manual
                .chain(snapshot.upcoming.iter().map(GuestTrack::from))
                .take(MAX_QUEUE)
                .collect(),
        }
    }
}
