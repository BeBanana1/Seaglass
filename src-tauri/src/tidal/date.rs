//! Horodatages de l'API, convertis en millisecondes depuis l'epoch Unix (UTC).
//!
//! Format observé (2026-10-30) pour `dateAdded` (titres de playlist) et `created`
//! (favoris) : `2024-04-12T22:51:56.872+0000`. Le décalage est aussi accepté sous la
//! forme `+00:00` ou `Z`, et les fractions de seconde sont facultatives.

/// Convertit un horodatage ISO 8601 en millisecondes UTC, `None` s'il est illisible.
pub(super) fn timestamp_ms(text: &str) -> Option<i64> {
    let text = text.trim();
    let (date, rest) = text.split_once('T')?;
    let mut date = date.splitn(3, '-');
    let year: i64 = date.next()?.parse().ok()?;
    let month: u32 = date.next()?.parse().ok()?;
    let day: u32 = date.next()?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    // Heure, puis décalage (`Z`, `±HHMM` ou `±HH:MM`).
    let offset_at = rest.find(['Z', 'z', '+', '-']).unwrap_or(rest.len());
    let (time, offset) = rest.split_at(offset_at);
    let (hms, fraction) = time.split_once('.').unwrap_or((time, ""));
    let mut hms = hms.splitn(3, ':');
    let hour: i64 = hms.next()?.parse().ok()?;
    let minute: i64 = hms.next()?.parse().ok()?;
    let second: i64 = hms.next().unwrap_or("0").parse().ok()?;
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let millis = fraction_ms(fraction)?;

    let offset_s = match offset {
        "" | "Z" | "z" => 0,
        _ => {
            let sign = if offset.starts_with('-') { -1 } else { 1 };
            let digits: String = offset[1..].chars().filter(|c| *c != ':').collect();
            if digits.len() != 4 {
                return None;
            }
            let h: i64 = digits[..2].parse().ok()?;
            let m: i64 = digits[2..].parse().ok()?;
            sign * (h * 3600 + m * 60)
        }
    };

    let seconds =
        days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second - offset_s;
    Some(seconds * 1000 + millis)
}

/// Minuit UTC d'une date `AAAA-MM-JJ` (`releaseDate` des albums), en millisecondes.
pub(super) fn date_ms(text: &str) -> Option<i64> {
    let mut parts = text.trim().splitn(3, '-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400_000)
}

/// Millisecondes d'une fraction de seconde (`872` → 872, `8` → 800), tronquée au-delà.
fn fraction_ms(fraction: &str) -> Option<i64> {
    if fraction.is_empty() {
        return Some(0);
    }
    if !fraction.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let padded = format!("{fraction:0<3}");
    padded[..3].parse().ok()
}

/// Nombre de jours depuis le 1970-01-01 (algorithme de Howard Hinnant, calendrier grégorien).
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let month = i64::from(month);
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_format() {
        // 2024-04-12T22:51:56.872Z
        assert_eq!(
            timestamp_ms("2024-04-12T22:51:56.872+0000"),
            Some(1_712_962_316_872)
        );
    }

    #[test]
    fn offsets_and_fractions() {
        assert_eq!(timestamp_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(timestamp_ms("1970-01-01T01:00:00+01:00"), Some(0));
        assert_eq!(timestamp_ms("1969-12-31T23:00:00-0100"), Some(0));
        assert_eq!(timestamp_ms("1970-01-01T00:00:01.5"), Some(1500));
        assert_eq!(timestamp_ms("2000-02-29T00:00:00Z"), Some(951_782_400_000));
    }

    #[test]
    fn garbage_is_rejected() {
        assert_eq!(timestamp_ms(""), None);
        assert_eq!(timestamp_ms("2024-04-12"), None);
        assert_eq!(timestamp_ms("2024-13-12T00:00:00Z"), None);
        assert_eq!(timestamp_ms("2024-04-12T00:00:00+1"), None);
        assert_eq!(timestamp_ms("2024-04-12T00:00:00.x"), None);
    }

    #[test]
    fn release_dates() {
        assert_eq!(date_ms("1970-01-02"), Some(86_400_000));
        assert_eq!(date_ms("2000-02-29"), Some(951_782_400_000));
        assert_eq!(date_ms("2024-13-01"), None);
        assert_eq!(date_ms(""), None);
    }
}
