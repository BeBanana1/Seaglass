//! Prérequis processeur du mpv embarqué (#92).
//!
//! L'installeur livre la build x86-64-v3 de mpv (zhongfly), qui exige l'AVX2 : sur un
//! processeur qui ne l'a pas (Intel avant Haswell, certains Pentium / Celeron / Atom), mpv
//! plante au lancement. L'app ne bloque rien d'avance : elle s'en sert seulement pour
//! expliquer un échec de mpv (`super::issue`).

/// Vrai si le processeur n'a pas l'AVX2.
#[cfg(windows)]
pub fn lacks_avx2() -> bool {
    use windows::Win32::System::Threading::{
        IsProcessorFeaturePresent, PF_AVX2_INSTRUCTIONS_AVAILABLE,
    };
    // SAFETY : fonction sans paramètre pointeur ni effet de bord.
    !unsafe { IsProcessorFeaturePresent(PF_AVX2_INSTRUCTIONS_AVAILABLE) }.as_bool()
}

/// Hors Windows, aucun mpv n'est embarqué : rien à expliquer.
#[cfg(not(windows))]
pub fn lacks_avx2() -> bool {
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn detection_does_not_panic() {
        // Le résultat dépend de la machine (les PC de dev et de CI ont l'AVX2).
        let _ = super::lacks_avx2();
    }
}
