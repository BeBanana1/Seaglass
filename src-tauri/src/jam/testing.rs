//! Faux lecteur des tests de la Jam (serveur local et relais).

use std::sync::Mutex;

use super::api::{Backend, Snapshot};
use super::guests::Guests;
use crate::tidal::models::{AlbumRef, ArtistRef, Track, TrackId};

#[derive(Default)]
pub struct Fake {
    pub added: Mutex<Vec<u64>>,
    pub named: Mutex<Vec<(u64, String)>>,
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

impl Backend for Fake {
    async fn search(&self, _query: String) -> Result<Vec<Track>, String> {
        Ok(vec![track(1), track(2)])
    }
    async fn enqueue(&self, id: TrackId) -> Result<(), String> {
        self.added.lock().unwrap().push(id.0);
        Ok(())
    }
    async fn state(&self) -> Result<Snapshot, String> {
        let added = self.added.lock().unwrap();
        Ok(Snapshot {
            now: Some(track(7)),
            paused: false,
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
