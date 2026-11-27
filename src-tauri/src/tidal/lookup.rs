//! Tracks by ISRC and albums by barcode (UPC), for links from other services pasted into
//! the Jam (#229). Verified on 2026-11-06: `tracks?isrc=USQX91300108` returns *Get Lucky*
//! (compilation first: the caller must redo the order), `albums?upc=886443927087` returns
//! *Random Access Memories* (20115556).

use super::Result;
use super::api::TidalClient;
use super::models::{AlbumSummary, Track};
use super::raw::{RawAlbumFull, RawPage, RawTrack};

impl TidalClient {
    /// Playable editions of a recording (original album, compilations…).
    pub async fn tracks_by_isrc(&self, isrc: &str) -> Result<Vec<Track>> {
        let params = [("isrc", isrc.to_owned()), ("limit", "20".to_owned())];
        let page: RawPage<RawTrack> = self.get("tracks", &params).await?;
        let tracks = page
            .items
            .into_iter()
            .filter(RawTrack::streamable)
            .filter_map(RawTrack::into_track)
            .collect();
        Ok(self.remember(tracks))
    }

    pub async fn albums_by_upc(&self, upc: &str) -> Result<Vec<AlbumSummary>> {
        let page: RawPage<RawAlbumFull> = self.get("albums", &[("upc", upc.to_owned())]).await?;
        Ok(page.items.into_iter().map(Into::into).collect())
    }

    pub async fn album_summary(&self, id: u64) -> Result<AlbumSummary> {
        let raw: RawAlbumFull = self.get(&format!("albums/{id}"), &[]).await?;
        Ok(raw.into())
    }
}
