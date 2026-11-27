//! Playback position as seen by guests (#448): pure logic, clock passed as an argument.
//!
//! The guests' state is pushed whenever its JSON changes (`push_loop`, one
//! read per second): a raw position would make it change every time. So we send
//! the instant (ms since the epoch) at which the track would have started, which only moves
//! on a track change, a pause, a seek or after a drift of more than `TOLERANCE_MS`.
//! The page derives the position from it with its own clock, corrected by the host's time
//! received with the lyrics.

/// Tolerated gap before moving the start point (position readings at ~4 Hz).
pub const TOLERANCE_MS: i64 = 1500;

/// Start point kept for a track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anchor {
    pub track: u64,
    pub started_at: i64,
}

/// Start point of track `track` played at `position` seconds at instant `now_ms`: the
/// previous one if it is still right within `TOLERANCE_MS`.
pub fn anchor(prev: Option<Anchor>, track: u64, position: f64, now_ms: i64) -> Anchor {
    let started_at = now_ms - (position.max(0.0) * 1000.0).round() as i64;
    match prev {
        Some(p) if p.track == track && (p.started_at - started_at).abs() <= TOLERANCE_MS => p,
        _ => Anchor { track, started_at },
    }
}

/// Now, in ms since the epoch.
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
        // Playback going on normally, readings slightly offset: same start point.
        assert_eq!(anchor(Some(first), 7, 15.4, 105_000), first);
        assert_eq!(anchor(Some(first), 7, 18.8, 110_000), first);
        // Seek within the track, or resume after a pause: new start point.
        assert_eq!(anchor(Some(first), 7, 60.0, 110_000).started_at, 50_000);
        assert_eq!(anchor(Some(first), 7, 15.0, 120_000).started_at, 105_000);
        // Other track, same apparent position: new point too.
        assert_eq!(anchor(Some(first), 8, 15.0, 105_000).track, 8);
    }
}
