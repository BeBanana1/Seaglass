//! Quand une écoute compte pour l'historique local (#202) : règle de Last.fm, le titre a
//! réellement joué la moitié de sa durée ou 4 minutes, et dure plus de 30 s. Le temps est
//! cumulé à partir des positions reçues du moteur : un saut en avant ne compte pas, une
//! pause non plus. Logique pure, sans horloge. Dit aussi quand annoncer le début d'un titre
//! à ListenBrainz (#431) : une fois par écoute, dès qu'il joue.

use crate::tidal::models::TrackId;

/// Titres plus courts : jamais comptés.
const MIN_DURATION: f64 = 30.0;
/// Seuil absolu, pour les longs titres.
const MAX_THRESHOLD: f64 = 240.0;
/// Écart maximal entre deux positions pour une lecture continue : au-delà, c'est un saut.
/// Android n'envoie la position que toutes les 5 s sans activité visible (#147).
const MAX_STEP: f64 = 6.0;
/// Retour au début (repeat One, « Précédent » après 3 s) : nouvelle écoute.
const RESTART_BELOW: f64 = 5.0;

#[derive(Debug, Default)]
pub struct ListenTracker {
    track: Option<TrackId>,
    last_pos: f64,
    listened: f64,
    counted: bool,
    announced: bool,
}

impl ListenTracker {
    /// Nouvelle position du titre courant (`duration` en secondes, 0 si inconnue). Renvoie le
    /// titre une seule fois par écoute, quand le seuil est franchi.
    pub fn on_position(&mut self, track: TrackId, duration: f64, pos: f64) -> Option<TrackId> {
        if self.track != Some(track) {
            *self = Self {
                track: Some(track),
                last_pos: pos,
                ..Self::default()
            };
            return None;
        }
        let step = pos - self.last_pos;
        if step < 0.0 && pos < RESTART_BELOW && self.last_pos - pos > RESTART_BELOW {
            self.listened = 0.0;
            self.counted = false;
            self.announced = false;
        } else if step > 0.0 && step <= MAX_STEP {
            self.listened += step;
        }
        self.last_pos = pos;
        if self.counted || duration <= MIN_DURATION {
            return None;
        }
        if self.listened >= (duration / 2.0).min(MAX_THRESHOLD) {
            self.counted = true;
            return Some(track);
        }
        None
    }

    /// Vrai une seule fois par écoute du titre courant (après `on_position`) : il joue,
    /// on peut annoncer son début.
    pub fn announce(&mut self, track: TrackId) -> bool {
        if self.track != Some(track) || self.announced {
            return false;
        }
        self.announced = true;
        true
    }

    /// Temps réellement écouté du titre courant, en secondes.
    pub fn listened(&self) -> f64 {
        self.listened
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: TrackId = TrackId(1);

    /// Positions de 0 à `to` par pas de `step`, renvoie le nombre d'écoutes comptées.
    fn play(tracker: &mut ListenTracker, duration: f64, from: f64, to: f64, step: f64) -> usize {
        let mut counted = 0;
        let mut pos = from;
        while pos <= to {
            counted += usize::from(tracker.on_position(T, duration, pos).is_some());
            pos += step;
        }
        counted
    }

    #[test]
    fn half_of_the_track() {
        let mut tracker = ListenTracker::default();
        assert_eq!(play(&mut tracker, 200.0, 0.0, 99.0, 0.25), 0);
        assert_eq!(play(&mut tracker, 200.0, 99.25, 200.0, 0.25), 1);
    }

    #[test]
    fn four_minutes_for_long_tracks() {
        let mut tracker = ListenTracker::default();
        assert_eq!(play(&mut tracker, 1200.0, 0.0, 239.0, 1.0), 0);
        assert_eq!(play(&mut tracker, 1200.0, 240.0, 600.0, 1.0), 1);
    }

    #[test]
    fn short_tracks_and_seeks_do_not_count() {
        let mut tracker = ListenTracker::default();
        assert_eq!(play(&mut tracker, 30.0, 0.0, 30.0, 0.25), 0);
        let mut tracker = ListenTracker::default();
        tracker.on_position(T, 200.0, 0.0);
        // Saut à la fin : pas d'écoute.
        assert_eq!(play(&mut tracker, 200.0, 150.0, 199.0, 1.0), 0);
    }

    #[test]
    fn slow_position_updates_still_count() {
        // Android en arrière-plan : une position toutes les 5 s.
        let mut tracker = ListenTracker::default();
        assert_eq!(play(&mut tracker, 200.0, 0.0, 200.0, 5.0), 1);
    }

    #[test]
    fn repeat_one_counts_again() {
        let mut tracker = ListenTracker::default();
        assert_eq!(play(&mut tracker, 100.0, 0.0, 100.0, 1.0), 1);
        assert_eq!(play(&mut tracker, 100.0, 0.0, 100.0, 1.0), 1);
    }

    #[test]
    fn announced_once_per_listen() {
        let mut tracker = ListenTracker::default();
        assert!(!tracker.announce(T));
        tracker.on_position(T, 100.0, 0.0);
        assert!(tracker.announce(T));
        assert!(!tracker.announce(T));
        play(&mut tracker, 100.0, 1.0, 100.0, 1.0);
        assert!(!tracker.announce(T));
        // Repeat One : nouvelle écoute, nouvelle annonce.
        tracker.on_position(T, 100.0, 0.0);
        assert!(tracker.announce(T));
        assert!(!tracker.announce(TrackId(2)));
    }

    #[test]
    fn a_new_track_resets() {
        let mut tracker = ListenTracker::default();
        play(&mut tracker, 200.0, 0.0, 90.0, 1.0);
        assert_eq!(tracker.on_position(TrackId(2), 200.0, 0.0), None);
        // L'écoute du premier titre ne se reporte pas sur le second.
        assert_eq!(tracker.on_position(TrackId(2), 200.0, 15.0), None);
    }
}
