//! Bandes originales de films (clin d'œil #399) : `models::Album` n'a ni genre ni type
//! « bande originale », donc une courte liste d'albums choisis à la main (décision de
//! l'utilisateur), sans aucune détection d'après le titre. Liste à entretenir : un album absent n'a simplement pas le
//! format large dans « En lecture ».
//!
//! Identifiants relevés dans la bibliothèque réelle (fixtures de l'aperçu, 2026-11-12).

const SOUNDTRACKS: &[u64] = &[
    156_749_593, // Interstellar (Expanded Edition)
    346_554_601, // Dune: Part Two
    346_598_849, // Dune: Part Two (autre édition)
    3_175_162,   // The Lord of the Rings: The Fellowship of the Ring
    77_608_387,  // Star Wars: The Force Awakens
    68_183_362,  // Rogue One: A Star Wars Story
    198_197,     // Blade Runner
    79_530_260,  // Blade Runner 2049
    66_762_455,  // Arrival
    157_109_463, // Tenet (Deluxe Edition)
    104_868_178, // Dunkirk
    105_419_824, // Batman Begins
    4_683_997,   // Gladiator
    5_476_745,   // Gladiator (autre édition)
    104_823_269, // Mad Max: Fury Road
    104_823_612, // Mad Max: Fury Road (Deluxe Version)
    165_842_024, // TRON: Legacy (The Complete Edition)
    130_237_089, // Oblivion
    104_823_359, // Gravity
    51_750_890,  // The Martian
    119_968_841, // Ad Astra
    133_185_658, // Ex Machina
    37_209_666,  // The Imitation Game
    132_026_880, // First Man
    3_701_303,   // Avatar
    130_227_158, // Alien: Covenant
    36_404_335,  // Star Trek Into Darkness
    445_499_744, // F1 The Movie
    445_499_760, // F1 The Movie (autre édition)
];

/// L'album fait partie des bandes originales choisies.
pub fn is_soundtrack(album: u64) -> bool {
    SOUNDTRACKS.contains(&album)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_albums_only() {
        assert!(is_soundtrack(156_749_593));
        assert!(is_soundtrack(3_175_162));
        assert!(!is_soundtrack(0));
        assert!(!is_soundtrack(1_234));
    }

    #[test]
    fn no_duplicates() {
        let mut ids = SOUNDTRACKS.to_vec();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), SOUNDTRACKS.len());
    }
}
