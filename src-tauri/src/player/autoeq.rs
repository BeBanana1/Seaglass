//! Import d'un profil AutoEQ (#428) : fichier « ParametricEQ.txt », le format d'Equalizer
//! APO que publie AutoEQ pour chaque casque.
//!
//! ```text
//! Preamp: -6.2 dB
//! Filter 1: ON LSC Fc 105 Hz Gain 5.5 dB Q 0.70
//! Filter 2: ON PK Fc 2386 Hz Gain -2.1 dB Q 1.26
//! Filter 10: ON HSC Fc 10000 Hz Gain -1.4 dB Q 0.70
//! ```
//!
//! Types acceptés : `PK` / `PEQ` (pic), `LSC` / `LS` (plateau grave), `HSC` / `HS`
//! (plateau aigu). Filtres `OFF` ignorés ; plateau sans `Q` : 0,71 (Butterworth). Toute
//! autre ligne (commentaire `#`, `Channel:`…) est ignorée. Le profil rendu a l'identifiant
//! 0 et un nom vide : l'UI les fixe avant de l'enregistrer.

use super::eq::{EqProfile, Filter, FilterKind, Invalid, MAX_FILTERS};
use crate::i18n::{tr, trf};

/// Taille maximale du texte collé : un vrai fichier fait moins d'un kilo-octet.
pub const MAX_LEN: usize = 16 * 1024;
const DEFAULT_SHELF_Q: f64 = 0.71;

/// Analyse le texte d'un fichier AutoEQ ; le profil rendu respecte les bornes de `eq`.
pub fn parse(text: &str) -> Result<EqProfile, Invalid> {
    if text.len() > MAX_LEN {
        return Err(not_autoeq());
    }
    let mut preamp = None;
    let mut filters = Vec::new();
    let mut n = 0;
    for line in text.lines().map(str::trim) {
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        if key == "preamp" {
            let value = rest.split_whitespace().next().and_then(number);
            preamp =
                Some(value.ok_or_else(|| {
                    Invalid(tr("préampli illisible", "unreadable preamp").into())
                })?);
        } else if key == "filter" || key.starts_with("filter ") {
            n += 1;
            if let Some(filter) = parse_filter(rest, n)? {
                filters.push(filter);
            }
        }
    }
    if filters.is_empty() {
        return Err(not_autoeq());
    }
    if filters.len() > MAX_FILTERS {
        return Err(Invalid(trf!(
            "{} filtres au plus",
            "{} filters at most",
            MAX_FILTERS
        )));
    }
    let profile = EqProfile {
        id: 0,
        name: String::new(),
        preamp,
        filters,
    };
    // Le nom, vide ici, est fixé par l'UI : seules les valeurs sont vérifiées.
    profile.check_values()?;
    Ok(profile)
}

fn not_autoeq() -> Invalid {
    Invalid(
        tr(
            "ce texte n'est pas un profil AutoEQ (ParametricEQ.txt)",
            "this text is not an AutoEQ profile (ParametricEQ.txt)",
        )
        .into(),
    )
}

/// Un filtre : `ON PK Fc 105 Hz Gain 3.4 dB Q 0.70`. `None` pour un filtre `OFF`.
fn parse_filter(rest: &str, n: usize) -> Result<Option<Filter>, Invalid> {
    let unreadable = || Invalid(trf!("filtre {} illisible", "filter {}: unreadable", n));
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    match tokens.first().map(|t| t.to_ascii_uppercase()).as_deref() {
        Some("ON") => {}
        Some("OFF") => return Ok(None),
        _ => return Err(unreadable()),
    }
    let kind = match tokens.get(1).map(|t| t.to_ascii_uppercase()).as_deref() {
        Some("PK" | "PEQ") => FilterKind::Peaking,
        Some("LSC" | "LS") => FilterKind::LowShelf,
        Some("HSC" | "HS") => FilterKind::HighShelf,
        Some(other) => {
            return Err(Invalid(trf!(
                "filtre {} : type {} non pris en charge",
                "filter {}: type {} is not supported",
                n,
                other
            )));
        }
        None => return Err(unreadable()),
    };
    let after = |name: &str| {
        tokens
            .iter()
            .position(|t| t.eq_ignore_ascii_case(name))
            .and_then(|i| tokens.get(i + 1))
            .and_then(|t| number(t))
    };
    let freq = after("Fc").ok_or_else(unreadable)?;
    let gain = after("Gain").ok_or_else(unreadable)?;
    let q = match (after("Q"), kind) {
        (Some(q), _) => q,
        (None, FilterKind::LowShelf | FilterKind::HighShelf) => DEFAULT_SHELF_Q,
        (None, FilterKind::Peaking) => return Err(unreadable()),
    };
    Ok(Some(Filter {
        kind,
        freq,
        gain,
        q,
    }))
}

/// Nombre décimal, virgule acceptée (export d'Equalizer APO en français).
fn number(token: &str) -> Option<f64> {
    token
        .replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Extrait d'un vrai fichier AutoEQ (Sennheiser HD 600, oratory1990).
    const HD600: &str = "Preamp: -6.2 dB
Filter 1: ON LSC Fc 105 Hz Gain 5.5 dB Q 0.70
Filter 2: ON PK Fc 2386 Hz Gain -2.1 dB Q 1.26
Filter 3: ON PK Fc 4483 Hz Gain 3.3 dB Q 3.31
Filter 4: ON PK Fc 159 Hz Gain -1.0 dB Q 0.44
Filter 5: ON PK Fc 7064 Hz Gain -2.2 dB Q 4.38
Filter 6: ON PK Fc 3346 Hz Gain -1.9 dB Q 4.44
Filter 7: ON PK Fc 1316 Hz Gain 1.1 dB Q 2.06
Filter 8: ON PK Fc 5550 Hz Gain 1.7 dB Q 5.95
Filter 9: ON PK Fc 19776 Hz Gain -2.6 dB Q 0.78
Filter 10: ON HSC Fc 10000 Hz Gain -1.4 dB Q 0.70
";

    #[test]
    fn reads_an_autoeq_file() {
        let p = parse(HD600).unwrap();
        assert_eq!(p.preamp, Some(-6.2));
        assert_eq!(p.filters.len(), 10);
        assert_eq!(
            p.filters[0],
            Filter {
                kind: FilterKind::LowShelf,
                freq: 105.0,
                gain: 5.5,
                q: 0.7
            }
        );
        assert_eq!(p.filters[1].kind, FilterKind::Peaking);
        assert_eq!(p.filters[9].kind, FilterKind::HighShelf);
        assert_eq!(p.filters[9].freq, 10_000.0);
        // Fins de ligne Windows et espaces en trop.
        let crlf = HD600.replace('\n', "\r\n  ");
        assert_eq!(parse(&crlf).unwrap(), p);
    }

    #[test]
    fn equalizer_apo_variants() {
        let p = parse(
            "# Correction\nChannel: All\nFilter: ON PEQ Fc 100 Hz Gain 2,5 dB Q 1,0\n\
             Filter: OFF PK Fc 200 Hz Gain 9 dB Q 1\nFilter: ON LS Fc 80 Hz Gain 3 dB\n",
        )
        .unwrap();
        assert_eq!(p.preamp, None, "préampli automatique");
        assert_eq!(p.filters.len(), 2, "filtre OFF ignoré");
        assert_eq!(p.filters[0].gain, 2.5);
        assert_eq!(p.filters[1].q, DEFAULT_SHELF_Q);
    }

    #[test]
    fn rejects_what_it_cannot_apply() {
        assert!(parse("").is_err());
        assert!(parse("bonjour").is_err());
        assert!(parse("Preamp: -3 dB\n").is_err(), "aucun filtre");
        assert!(parse("Preamp: abc dB\nFilter 1: ON PK Fc 100 Hz Gain 1 dB Q 1").is_err());
        let notch = parse("Filter 1: ON NO Fc 100 Hz").unwrap_err();
        assert!(notch.0.contains("NO"), "{notch}");
        assert!(
            parse("Filter 1: ON PK Fc 100 Hz Gain 1 dB").is_err(),
            "pic sans Q"
        );
        assert!(
            parse("Filter 1: ON PK Fc 5 Hz Gain 1 dB Q 1").is_err(),
            "hors bornes"
        );
        assert!(parse("Filter 1: ON PK Fc 100 Hz Gain 30 dB Q 1").is_err());
        let many: String = (1..=MAX_FILTERS + 1)
            .map(|i| format!("Filter {i}: ON PK Fc 100 Hz Gain 1 dB Q 1\n"))
            .collect();
        assert!(parse(&many).is_err());
        assert!(parse(&"x".repeat(MAX_LEN + 1)).is_err());
    }
}
