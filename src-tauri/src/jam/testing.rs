//! Faux lecteur des tests de la Jam (serveur local et relais).

use std::sync::Mutex;

use super::api::{Backend, Hits, Snapshot};
use super::guests::Guests;
use crate::tidal::models::{AlbumRef, AlbumSummary, ArtistRef, Track, TrackId};

#[derive(Default)]
pub struct Fake {
    pub added: Mutex<Vec<u64>>,
    pub named: Mutex<Vec<(u64, String)>>,
    pub paused: Mutex<Option<bool>>,
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
    async fn state(&self) -> Result<Snapshot, String> {
        let added = self.added.lock().unwrap();
        Ok(Snapshot {
            now: Some(track(7)),
            paused: self.paused.lock().unwrap().unwrap_or(false),
            manual: added.iter().map(|id| track(*id)).collect(),
            upcoming: (8..50).map(track).collect(),
        })
    }
    fn guests_changed(&self, guests: &Guests) {
        let mut named: Vec<_> = guests.snapshot().into_iter().collect();
        named.sort();
        *self.named.lock().unwrap() = named;
    }
}
