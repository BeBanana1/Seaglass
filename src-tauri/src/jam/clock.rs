//! Position de lecture vue par les invités (#448) : logique pure, horloge passée en argument.
//!
//! L'état des invités est poussé chaque fois que son JSON change (`push_loop`, une
//! relecture par seconde) : une position brute le ferait changer à chaque fois. On envoie
//! donc l'instant (ms depuis l'epoch) où le titre aurait commencé, qui ne bouge qu'au
//! changement de titre, à une pause, à un saut ou après une dérive de plus de `TOLERANCE_MS`.
//! La page en déduit la position avec son horloge, corrigée par l'heure de l'hôte reçue avec
//! les paroles.

/// Écart toléré avant de déplacer le point de départ (relevés de position à ~4 Hz).
pub const TOLERANCE_MS: i64 = 1500;

/// Point de départ retenu pour un titre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anchor {
    pub track: u64,
    pub started_at: i64,
}

/// Point de départ du titre `track` lu à `position` secondes à l'instant `now_ms` : le
/// précédent s'il est encore juste à `TOLERANCE_MS` près.
pub fn anchor(prev: Option<Anchor>, track: u64, position: f64, now_ms: i64) -> Anchor {
    let started_at = now_ms - (position.max(0.0) * 1000.0).round() as i64;
    match prev {
        Some(p) if p.track == track && (p.started_at - started_at).abs() <= TOLERANCE_MS => p,
        _ => Anchor { track, started_at },
    }
}

/// Maintenant, en ms depuis l'epoch.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_anchor_moves_only_on_a_real_change() {
        let first = anchor(None, 7, 10.0, 100_000);
        assert_eq!(first.started_at, 90_000);
        // Lecture qui suit son cours, relevés un peu décalés : même point de départ.
        assert_eq!(anchor(Some(first), 7, 15.4, 105_000), first);
        assert_eq!(anchor(Some(first), 7, 18.8, 110_000), first);
        // Saut dans le titre, ou reprise après une pause : nouveau point de départ.
        assert_eq!(anchor(Some(first), 7, 60.0, 110_000).started_at, 50_000);
        assert_eq!(anchor(Some(first), 7, 15.0, 120_000).started_at, 105_000);
        // Autre titre, même position apparente : nouveau point aussi.
        assert_eq!(anchor(Some(first), 8, 15.0, 105_000).track, 8);
    }
}
