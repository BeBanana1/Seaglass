//! Filtre du contenu proposé par le service : titres et albums explicites (#368) ou générés
//! par IA (#369, masqués par défaut) retirés des listes.
//!
//! Le filtre s'applique à ce que le service propose (recherche, pages album / artiste,
//! playlists des autres, mixes, radios, accueil, fil d'actualité, Jam, radio automatique),
//! jamais à ce que l'utilisateur a enregistré lui-même (favoris, ses propres playlists) :
//! il les a choisis, et les modifications d'une playlist passent par la position des titres.
//!
//! Le réglage vit dans `UiSettings::content` ; sa copie globale, mise à jour au démarrage et
//! à chaque enregistrement, évite de relire SQLite à chaque liste (et sert hors des
//! commandes : radio automatique de l'acteur, Jam).

use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use crate::tidal::models::{
    AlbumPage, AlbumSummary, ArtistExtra, ArtistPage, FeedItem, HomeRow, MediaItem, MixPage,
    PlaylistPage, RadioPage, SearchResults, Track,
};

// Valeurs de `ContentFilter::default()` jusqu'au premier `apply`.
static HIDE_EXPLICIT: AtomicBool = AtomicBool::new(false);
static HIDE_AI: AtomicBool = AtomicBool::new(true);

/// Réglage « contenu » : ce qui est masqué dans ce que le service propose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ContentFilter {
    /// Masque les titres et albums marqués explicites (`explicit`). Désactivé par défaut.
    pub hide_explicit: bool,
    /// Masque les titres et albums générés par IA selon le service (`ai`, #369). Activé par
    /// défaut, réglages enregistrés avant la 0.5.29 compris.
    pub hide_ai: bool,
}

impl Default for ContentFilter {
    fn default() -> Self {
        Self {
            hide_explicit: false,
            hide_ai: true,
        }
    }
}

impl ContentFilter {
    /// Filtre en vigueur.
    pub fn current() -> Self {
        Self {
            hide_explicit: HIDE_EXPLICIT.load(Ordering::Relaxed),
            hide_ai: HIDE_AI.load(Ordering::Relaxed),
        }
    }

    /// Rend ce réglage effectif pour les listes suivantes.
    pub fn apply(self) {
        HIDE_EXPLICIT.store(self.hide_explicit, Ordering::Relaxed);
        HIDE_AI.store(self.hide_ai, Ordering::Relaxed);
    }

    pub fn allows_track(&self, track: &Track) -> bool {
        !(self.hide_explicit && track.explicit || self.hide_ai && track.ai)
    }

    pub fn allows_album(&self, album: &AlbumSummary) -> bool {
        !(self.hide_explicit && album.explicit || self.hide_ai && album.ai)
    }

    fn allows_item(&self, item: &MediaItem) -> bool {
        match item {
            MediaItem::Album(album) => self.allows_album(album),
            MediaItem::Artist(_) | MediaItem::Playlist(_) | MediaItem::Mix(_) => true,
        }
    }

    fn tracks(&self, tracks: &mut Vec<Track>) {
        tracks.retain(|t| self.allows_track(t));
    }

    fn albums(&self, albums: &mut Vec<AlbumSummary>) {
        albums.retain(|a| self.allows_album(a));
    }
}

/// Liste ou page dont on retire ce que le filtre masque.
pub trait Filtered {
    fn filter_with(&mut self, filter: &ContentFilter);
}

/// `value` sans ce que le filtre en vigueur masque.
pub fn filtered<T: Filtered>(mut value: T) -> T {
    value.filter_with(&ContentFilter::current());
    value
}

impl Filtered for Vec<Track> {
    fn filter_with(&mut self, filter: &ContentFilter) {
        filter.tracks(self);
    }
}

impl Filtered for Vec<AlbumSummary> {
    fn filter_with(&mut self, filter: &ContentFilter) {
        filter.albums(self);
    }
}

impl Filtered for Option<AlbumSummary> {
    fn filter_with(&mut self, filter: &ContentFilter) {
        if self.as_ref().is_some_and(|a| !filter.allows_album(a)) {
            *self = None;
        }
    }
}

impl Filtered for SearchResults {
    fn filter_with(&mut self, filter: &ContentFilter) {
        filter.tracks(&mut self.tracks);
        filter.albums(&mut self.albums);
    }
}

/// L'album lui-même reste affiché (on y arrive par un lien ou un favori) : seuls ses
/// titres masqués disparaissent.
impl Filtered for AlbumPage {
    fn filter_with(&mut self, filter: &ContentFilter) {
        filter.tracks(&mut self.tracks);
    }
}

impl Filtered for ArtistPage {
    fn filter_with(&mut self, filter: &ContentFilter) {
        filter.tracks(&mut self.top_tracks);
        filter.albums(&mut self.albums);
        filter.albums(&mut self.singles);
    }
}

impl Filtered for ArtistExtra {
    fn filter_with(&mut self, filter: &ContentFilter) {
        filter.albums(&mut self.appears_on);
    }
}

/// Les playlists de l'utilisateur restent complètes (voir l'en-tête du module).
impl Filtered for PlaylistPage {
    fn filter_with(&mut self, filter: &ContentFilter) {
        if !self.playlist.mine {
            self.tracks.retain(|e| filter.allows_track(&e.track));
        }
    }
}

impl Filtered for MixPage {
    fn filter_with(&mut self, filter: &ContentFilter) {
        filter.tracks(&mut self.tracks);
    }
}

impl Filtered for RadioPage {
    fn filter_with(&mut self, filter: &ContentFilter) {
        filter.tracks(&mut self.tracks);
    }
}

/// Une ligne vidée par le filtre disparaît ; une ligne déjà vide reste telle quelle.
impl Filtered for Vec<HomeRow> {
    fn filter_with(&mut self, filter: &ContentFilter) {
        self.retain_mut(|row| {
            let before = row.items.len();
            row.items.retain(|i| filter.allows_item(i));
            before == 0 || !row.items.is_empty()
        });
    }
}

impl Filtered for Vec<FeedItem> {
    fn filter_with(&mut self, filter: &ContentFilter) {
        self.retain(|f| filter.allows_item(&f.item));
    }
}

impl Filtered for Option<PlaylistPage> {
    fn filter_with(&mut self, filter: &ContentFilter) {
        if let Some(page) = self {
            page.filter_with(filter);
        }
    }
}

#[cfg(test)]
mod tests;
