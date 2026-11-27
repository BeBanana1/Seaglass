//! UTC dates without a date crate (H. Hinnant's algorithm): crash reports (#192)
//! and library export (#330).

/// UTC date and time for a number of seconds since the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Utc {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl Utc {
    pub fn from_secs(secs: i64) -> Self {
        let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        // Bounded by construction: day 1–31, month 1–12, remainder < 86,400.
        Self {
            year: yoe + era * 400 + i64::from(month <= 2),
            month: month as u32,
            day: day as u32,
            hour: (rem / 3600) as u32,
            minute: (rem % 3600 / 60) as u32,
            second: (rem % 60) as u32,
        }
    }

    /// Now (the epoch if the clock is earlier). Only the export (desktop) uses it.
    #[cfg(desktop)]
    pub fn now() -> Self {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        Self::from_secs(i64::try_from(secs).unwrap_or(0))
    }

    /// `YYYY-MM-DD`.
    pub fn date(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// `hh:mm:ss`.
    pub fn time(&self) -> String {
        format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
    }

    /// ISO 8601: `YYYY-MM-DDThh:mm:ssZ`.
    #[cfg(any(desktop, test))]
    pub fn iso(&self) -> String {
        format!("{}T{}Z", self.date(), self.time())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dates() {
        assert_eq!(Utc::from_secs(0).iso(), "1970-01-01T00:00:00Z");
        assert_eq!(Utc::from_secs(1_793_276_645).iso(), "2026-10-29T12:24:05Z");
        // 29 February 2000.
        assert_eq!(Utc::from_secs(951_782_400).date(), "2000-02-29");
        assert_eq!(Utc::from_secs(-1).iso(), "1969-12-31T23:59:59Z");
    }
}
