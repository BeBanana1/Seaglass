//! Film soundtracks (nod #399): `models::Album` has no genre or "soundtrack" type, so
//! this is a short list of albums picked by hand (the user's decision), with no detection
//! from the title. A list to maintain: a missing album simply does not get the wide
//! format in "Now playing".
//!
//! Ids taken from the real library (preview fixtures, 2026-11-12).

const SOUNDTRACKS: &[u64] = &[
    156_749_593, // Interstellar (Expanded Edition)
    346_554_601, // Dune: Part Two
    346_598_849, // Dune: Part Two (other edition)
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
    5_476_745,   // Gladiator (other edition)
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
    445_499_760, // F1 The Movie (other edition)
];

/// The album is one of the chosen soundtracks.
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
