//! Faux lecteur des tests de la Jam (serveur local et relais).

use std::sync::Mutex;

use super::api::{Backend, Hits, Snapshot};
use super::guests::Guests;
use crate::player::blind::{Answer, BlindTestView, Mode, Party, Phase, PlayerView, Refusal};
use crate::player::volume::Volume;
use crate::tidal::models::{AlbumRef, AlbumSummary, ArtistRef, Track, TrackId};

#[derive(Default)]
pub struct Fake {
    pub added: Mutex<Vec<u64>>,
    pub named: Mutex<Vec<(u64, String)>>,
    pub paused: Mutex<Option<bool>>,
    /// Blind test en cours (#441) ; les réponses reçues, dans l'ordre.
    pub blind: Mutex<Option<BlindTestView>>,
    pub answers: Mutex<Vec<GuestAnswer>>,
    /// Volume de l'hôte (#449).
    pub volume: Mutex<Volume>,
}

/// Réponse reçue par le faux lecteur : clé, prénom, manche, réponse.
pub type GuestAnswer = (String, Option<String>, usize, Answer);

/// Maintenant, en ms depuis l'epoch.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// Blind test de la manche 2 sur 5, ouvert aux invités si `jam`.
pub fn blind_game(jam: bool) -> BlindTestView {
    BlindTestView {
        title: "Playlist".into(),
        mode: Mode::Typing,
        phase: Phase::Guess,
        round: 2,
        rounds: 5,
        score: 0,
        max_score: 15,
        streak: 0,
        best_streak: 0,
        clip_start: 40.0,
        clip_length: 15.0,
        choices: Vec::new(),
        results: Vec::new(),
        party: Party {
            jam,
            host_plays: true,
        },
        players: vec![PlayerView {
            id: 0,
            name: None,
            host: true,
            score: 3,
            streak: 1,
            best_streak: 1,
            answered: false,
        }],
        reveal_at: Some(now_ms() + 30_000),
        clip_playing: true,
    }
}

pub fn track(id: u64) -> Track {
    Track {
        id: TrackId(id),
        title: format!("Titre {id}"),
        version: (id == 2).then(|| "Live".into()),
        duration: 200,
        artists: vec![
            ArtistRef {
                id: 1,
                name: "A".into(),
            },
            ArtistRef {
                id: 2,
                name: "B".into(),
            },
        ],
        album: AlbumRef {
            id: 9,
            title: "Album".into(),
            cover_url: None,
            cover_small_url: Some("https://resources.tidal.com/x.jpg".into()),
            cover_large_url: None,
            color: None,
        },
        explicit: false,
        ai: false,
        hires: false,
        isrc: None,
    }
}

pub fn album(id: u64) -> AlbumSummary {
    AlbumSummary {
        id,
        title: format!("Album {id}"),
        artists: vec![ArtistRef {
            id: 1,
            name: "A".into(),
        }],
        cover_url: None,
        cover_large_url: None,
        color: None,
        release_date: Some("2013-05-17".into()),
        number_of_tracks: Some(35),
        album_type: Some("ALBUM".into()),
        explicit: false,
        ai: false,
        hires: false,
    }
}

impl Backend for Fake {
    async fn search(&self, _query: String) -> Result<Hits, String> {
        Ok(Hits {
            tracks: vec![track(1), track(2)],
            albums: vec![album(40)],
        })
    }
    /// Tout lien collé désigne l'album 41.
    async fn link(&self, _text: String) -> Result<Hits, String> {
        Ok(Hits {
            tracks: Vec::new(),
            albums: vec![album(41)],
        })
    }
    /// L'album 40 a 35 titres (100 à 134) : plus que le plafond d'un ajout.
    async fn album_tracks(&self, id: u64) -> Result<Vec<Track>, String> {
        Ok(if id == 40 {
            (100..135).map(track).collect()
        } else {
            Vec::new()
        })
    }
    async fn enqueue(&self, ids: Vec<TrackId>) -> Result<(), String> {
        self.added.lock().unwrap().extend(ids.iter().map(|id| id.0));
        Ok(())
    }
    async fn remove(&self, ids: Vec<TrackId>) -> Result<(), String> {
        let mut added = self.added.lock().unwrap();
        for id in ids {
            if let Some(i) = added.iter().position(|a| *a == id.0) {
                added.remove(i);
            }
        }
        Ok(())
    }
    async fn set_paused(&self, paused: bool) -> Result<(), String> {
        *self.paused.lock().unwrap() = Some(paused);
        Ok(())
    }
    async fn set_volume(&self, level: u8) -> Result<(), String> {
        self.volume.lock().unwrap().set_level(level);
        Ok(())
    }
    async fn state(&self) -> Result<Snapshot, String> {
        let added = self.added.lock().unwrap();
        Ok(Snapshot {
            now: Some(track(7)),
            paused: self.paused.lock().unwrap().unwrap_or(false),
            manual: added.iter().map(|id| track(*id)).collect(),
            upcoming: (8..50).map(track).collect(),
            blind: self.blind.lock().unwrap().clone(),
            volume: *self.volume.lock().unwrap(),
        })
    }
    /// Comme l'acteur : refusé sans partie ouverte aux invités, pour une autre manche ou en
    /// double ; l'invité reçoit l'identifiant 1, l'autre 2.
    async fn blind_answer(
        &self,
        key: String,
        name: Option<String>,
        round: usize,
        answer: Answer,
    ) -> Result<Result<u32, Refusal>, String> {
        let open = self
            .blind
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|b| b.party.jam && b.round == round);
        if !open {
            return Ok(Err(Refusal::Closed));
        }
        let mut answers = self.answers.lock().unwrap();
        if answers.iter().any(|(k, ..)| *k == key) {
            return Ok(Err(Refusal::Duplicate));
        }
        answers.push((key, name, round, answer));
        Ok(Ok(answers.len() as u32))
    }
    fn guests_changed(&self, guests: &Guests) {
        let mut named: Vec<_> = guests.snapshot().into_iter().collect();
        named.sort();
        *self.named.lock().unwrap() = named;
    }
}
