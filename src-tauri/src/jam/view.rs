//! What the guests' page receives (#25): tracks, albums and state, nothing else leaves
//! the app.

use serde::Serialize;

use super::api::{GuestSkip, MAX_QUEUE, Snapshot};
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

/// Track as the page shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestTrack {
    pub id: u64,
    pub title: String,
    pub artists: String,
    pub album: String,
    pub cover: Option<String>,
    pub duration: u32,
    /// Guest who added it to the manual queue.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
    /// Jam batch that added it (#223): its author's page offers to remove it.
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

/// Album from a Jam search (#223).
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
    /// Guests can pause and resume (#222, the host's choice).
    pub controls: bool,
    /// Guests can add (the host can suspend additions, #288).
    pub adds: bool,
    /// Guests can add a whole album (#288).
    pub albums: bool,
    /// Pending tracks at most per guest, if the host set a limit (#288).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Manual queue (with the guest's name) then the rest of the context, `MAX_QUEUE` at most.
    pub queue: Vec<GuestTrack>,
    /// Blind test open to guests (#441).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blind: Option<GuestBlind>,
    /// Volume, only if the host lets guests set it (#449).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<GuestVolume>,
    /// Instant (ms since the epoch, host's time) when the current track would have started,
    /// while playing (#448, `clock.rs`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<i64>,
    /// Position in ms, while paused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<u64>,
    /// The current track has lyrics; absent until a guest has asked for them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lyrics: Option<bool>,
    /// Vote to skip the current track (#433), only if the host allows it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip: Option<GuestSkip>,
}

/// Volume as the guests see it (#449).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestVolume {
    pub level: u8,
    /// The host's cap: the slider does not go higher.
    pub max: u8,
    /// Sound muted by the host: the slider is greyed out.
    pub muted: bool,
}

/// Blind test as the guests see it (#441): never the answer before the reveal, nor the
/// players' entries.
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
    /// Ranking, with who answered the current round.
    pub players: Vec<PlayerView>,
    /// Planned automatic reveal (ms since the epoch).
    pub reveal_at: Option<i64>,
    /// Last revealed round: its track and everyone's result.
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
            volume: rules.guest_volume.then_some(GuestVolume {
                level: snapshot.volume.level,
                max: rules.guest_volume_max,
                muted: snapshot.volume.muted,
            }),
            started_at: None,
            position: None,
            lyrics: None,
            skip: None,
        }
    }
}
