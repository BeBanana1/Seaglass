//! A file's `af` chain: normalization gain (#82) and equalizer preamp in a single `volume`
//! filter, then the equalizer's filters (#428). Pure tested logic.
//!
//! The chain is always set **per file** (`af` option of `loadfile`, then
//! `file-local-options/af`): a file-local value replaces the whole `af` list, so a global
//! equalizer would be lost on every normalized track.

use crate::engine::Filters;
use crate::player::eq::{EqProfile, FilterKind};

/// Value of `af` (`volume=volume=-6.53dB,equalizer=f=105:t=q:w=0.70:g=3.40`), empty
/// without a filter. libavfilter filters: `equalizer` (peak), `bass` / `treble` (shelves),
/// with the width given as Q (`t=q`), like AutoEQ's RBJ biquads.
pub fn af_value(filters: &Filters) -> String {
    let eq = filters.eq.as_deref();
    let volume = filters.gain.unwrap_or(0.0) + eq.map_or(0.0, EqProfile::effective_preamp);
    let volume = (volume * 100.0).round() / 100.0;
    let mut parts = Vec::new();
    if volume != 0.0 {
        parts.push(format!("volume=volume={volume:.2}dB"));
    }
    for f in eq.map(|p| p.filters.as_slice()).unwrap_or_default() {
        if f.gain == 0.0 {
            continue;
        }
        let name = match f.kind {
            FilterKind::Peaking => "equalizer",
            FilterKind::LowShelf => "bass",
            FilterKind::HighShelf => "treble",
        };
        parts.push(format!(
            "{name}=f={:.1}:t=q:w={:.3}:g={:.2}",
            f.freq, f.q, f.gain
        ));
    }
    parts.join(",")
}

/// File options of `loadfile` (the `options` argument, comma-separated): start position
/// and `af` chain. `None` if there are none. A chain of several filters contains commas: it
/// is passed as `%length%value`, the unambiguous form of mpv's option lists.
pub fn file_options(start: f64, filters: &Filters) -> Option<String> {
    let mut options = Vec::new();
    if start > 0.0 {
        options.push(format!("start={start}"));
    }
    let af = af_value(filters);
    if !af.is_empty() {
        if af.contains(',') {
            options.push(format!("af=%{}%{af}", af.len()));
        } else {
            options.push(format!("af={af}"));
        }
    }
    (!options.is_empty()).then(|| options.join(","))
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::player::eq::Filter;

    pub(crate) fn hd600() -> Arc<EqProfile> {
        Arc::new(EqProfile {
            id: 1,
            name: "HD 600".into(),
            preamp: Some(-6.2),
            filters: vec![
                Filter {
                    kind: FilterKind::LowShelf,
                    freq: 105.0,
                    gain: 5.5,
                    q: 0.7,
                },
                Filter {
                    kind: FilterKind::Peaking,
                    freq: 2386.0,
                    gain: -2.1,
                    q: 1.26,
                },
                Filter {
                    kind: FilterKind::HighShelf,
                    freq: 10_000.0,
                    gain: -1.4,
                    q: 0.7,
                },
            ],
        })
    }

    fn gain(g: Option<f64>) -> Filters {
        Filters { gain: g, eq: None }
    }

    #[test]
    fn gain_only_is_unchanged() {
        assert_eq!(af_value(&gain(Some(-6.5))), "volume=volume=-6.50dB");
        assert_eq!(af_value(&gain(None)), "");
        assert_eq!(file_options(0.0, &gain(None)), None);
        assert_eq!(
            file_options(12.5, &gain(None)).as_deref(),
            Some("start=12.5")
        );
        assert_eq!(
            file_options(12.5, &gain(Some(3.1))).as_deref(),
            Some("start=12.5,af=volume=volume=3.10dB")
        );
        assert_eq!(
            file_options(0.0, &gain(Some(-1.0))).as_deref(),
            Some("af=volume=volume=-1.00dB")
        );
    }

    #[test]
    fn equalizer_chain_after_one_volume() {
        let chain = "volume=volume=-12.70dB,bass=f=105.0:t=q:w=0.700:g=5.50,\
                     equalizer=f=2386.0:t=q:w=1.260:g=-2.10,treble=f=10000.0:t=q:w=0.700:g=-1.40";
        let filters = Filters {
            gain: Some(-6.5),
            eq: Some(hd600()),
        };
        assert_eq!(af_value(&filters), chain, "gain and preamp merged");
        assert_eq!(
            file_options(42.0, &filters),
            Some(format!("start=42,af=%{}%{chain}", chain.len()))
        );
        // Without normalization (turned off, or exclusive mode): preamp alone.
        let eq_only = Filters {
            gain: None,
            eq: Some(hd600()),
        };
        assert!(af_value(&eq_only).starts_with("volume=volume=-6.20dB,bass="));
    }

    #[test]
    fn neutral_parts_are_left_out() {
        // The gain exactly offsets the preamp, and a 0 dB filter does nothing.
        let mut p = (*hd600()).clone();
        p.filters[1].gain = 0.0;
        let filters = Filters {
            gain: Some(6.2),
            eq: Some(Arc::new(p)),
        };
        let af = af_value(&filters);
        assert!(af.starts_with("bass="), "{af}");
        assert!(!af.contains("equalizer"), "{af}");
    }
}
