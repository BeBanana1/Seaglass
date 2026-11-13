//! Ce que la page des invités reçoit (#25) : titres, albums et état, rien d'autre ne sort
//! de l'app.

use serde::Serialize;

use super::api::{MAX_QUEUE, Snapshot};
use super::guests::Guests;
use super::rules::JamRules;
use crate::player::blind::{Choice, Mode, Phase, PlayerResult, PlayerView};
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
    /// Blind test ouvert aux invités (#441).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blind: Option<GuestBlind>,
}

/// Blind test vu par les invités (#441) : jamais la réponse avant la révélation, ni les
/// saisies des joueurs.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestBlind {
    pub title: String,
    pub mode: Mode,
    pub phase: Phase,
    pub round: usize,
    pub rounds: usize,
    pub clip_start: f64,
    pub clip_length: f64,
    pub choices: Vec<Choice>,
    /// Classement, avec qui a répondu à la manche en cours.
    pub players: Vec<PlayerView>,
    /// Révélation automatique prévue (ms depuis l'epoch).
    pub reveal_at: Option<i64>,
    /// Dernière manche révélée : son titre et le résultat de chacun.
    pub last: Option<GuestRound>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestRound {
    pub round: usize,
    pub track: GuestTrack,
    pub players: Vec<PlayerResult>,
}

impl GuestBlind {
    fn new(view: &crate::player::blind::BlindTestView) -> Self {
        let last = (view.phase != Phase::Guess)
            .then(|| view.results.last())
            .flatten()
            .map(|r| GuestRound {
                round: view.results.len(),
                track: GuestTrack::from(&r.track),
                players: r.players.clone(),
            });
        Self {
            title: view.title.clone(),
            mode: view.mode,
            phase: view.phase,
            round: view.round,
            rounds: view.rounds,
            clip_start: view.clip_start,
            clip_length: view.clip_length,
            choices: view.choices.clone(),
            players: view.players.clone(),
            reveal_at: view.reveal_at,
            last,
        }
    }
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
            adds: adds && snapshot.jam_game().is_none(),
            albums: rules.albums,
            limit: rules.max_pending,
            queue: manual
                .chain(snapshot.upcoming.iter().map(GuestTrack::from))
                .take(MAX_QUEUE)
                .collect(),
            blind: snapshot.jam_game().map(GuestBlind::new),
        }
    }
}
