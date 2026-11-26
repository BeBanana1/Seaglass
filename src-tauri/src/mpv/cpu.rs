//! Processor requirement of the bundled mpv (#92).
//!
//! The installer ships the x86-64-v3 build of mpv (zhongfly), which requires AVX2: on a
//! processor without it (Intel before Haswell, some Pentium / Celeron / Atom), mpv crashes
//! at launch. The app blocks nothing up front: it only uses this to explain an mpv failure
//! (`super::issue`).

/// True if the processor lacks AVX2.
#[cfg(windows)]
pub fn lacks_avx2() -> bool {
    use windows::Win32::System::Threading::{
        IsProcessorFeaturePresent, PF_AVX2_INSTRUCTIONS_AVAILABLE,
    };
    // SAFETY: function with no pointer parameter and no side effect.
    !unsafe { IsProcessorFeaturePresent(PF_AVX2_INSTRUCTIONS_AVAILABLE) }.as_bool()
}

/// Outside Windows, no mpv is bundled: nothing to explain.
#[cfg(not(windows))]
pub fn lacks_avx2() -> bool {
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn detection_does_not_panic() {
        // The result depends on the machine (the dev and CI machines have AVX2).
        let _ = super::lacks_avx2();
    }
}
