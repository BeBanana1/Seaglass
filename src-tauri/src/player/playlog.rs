//! Listens for TIDAL's play log (#34): one session per stream heard, from the first
//! sound to the last, with pauses and seeks as actions, as the official SDKs record them
//! (`tidal-sdk-web` `basePlayer.ts`). Pure logic, the clock and new ids are passed in;
//! sending is in `report.rs`.
//!
//! - A stream loaded at entry 0 is *pending* until a position arrives while playing: the
//!   session starts there, at the load position.
//! - Gapless (`advanced`): the previous session ends at its duration, the next one starts
//!   at 0 at the same instant (SDK test `case-gapless`).
//! - Repeat One loops without reloading: going back near 0 from near the end ends the
//!   session at its duration and starts a new one, under a fresh id that no stream request
//!   carried (unverified on TIDAL's side).

use crate::tidal::models::{LogAction, LogSource, PlayLog, PlaybackSession};

/// A position below this, coming from above `LOOP_FROM` of the end, is a loop.
const LOOP_TO: f64 = 5.0;
const LOOP_FROM: f64 = 5.0;

#[derive(Debug, Default)]
pub struct PlayLogTracker {
    pending: Option<Pending>,
    open: Option<Open>,
}

#[derive(Debug)]
struct Pending {
    session: PlaybackSession,
    source: LogSource,
    start: f64,
}

#[derive(Debug)]
struct Open {
    log: PlayLog,
    last_pos: f64,
    paused: bool,
}

impl Open {
    fn new(session: PlaybackSession, source: LogSource, start: f64, now: i64) -> Self {
        Self {
            log: PlayLog {
                session,
                source,
                start_timestamp: now,
                start_position: start,
                end_timestamp: now,
                end_position: start,
                actions: Vec::new(),
            },
            last_pos: start,
            paused: false,
        }
    }

    fn end(mut self, position: f64, now: i64) -> PlayLog {
        self.log.end_position = position.max(self.log.start_position);
        self.log.end_timestamp = now.max(self.log.start_timestamp);
        self.log
            .actions
            .retain(|a| a.timestamp <= self.log.end_timestamp);
        self.log
    }

    fn action(&mut self, start: bool, position: f64, now: i64) {
        self.log.actions.push(LogAction {
            start,
            position,
            timestamp: now,
        });
    }
}

impl PlayLogTracker {
    /// A stream was loaded at entry 0 at `start` seconds. Ends the open session (another
    /// track, or the same one reloaded) at its last position.
    pub fn loaded(
        &mut self,
        session: PlaybackSession,
        source: LogSource,
        start: f64,
        now: i64,
    ) -> Option<PlayLog> {
        let ended = self.close(now);
        self.pending = Some(Pending {
            session,
            source,
            start,
        });
        ended
    }

    /// The engine moved to the preloaded stream: the previous session ends at
    /// `previous_duration`, the next one starts now at 0.
    pub fn advanced(
        &mut self,
        session: PlaybackSession,
        source: LogSource,
        previous_duration: f64,
        now: i64,
    ) -> Option<PlayLog> {
        self.pending = None;
        let ended = self
            .open
            .take()
            .map(|open| open.end(previous_duration, now));
        self.open = Some(Open::new(session, source, 0.0, now));
        ended
    }

    /// New position of the current stream. Starts a pending session if playing; on a
    /// repeat One loop, returns the ended listen and starts the next under `new_id()`.
    pub fn position(
        &mut self,
        pos: f64,
        duration: f64,
        paused: bool,
        now: i64,
        new_id: impl FnOnce() -> String,
    ) -> Option<PlayLog> {
        if let Some(open) = &mut self.open {
            let looped = duration > 0.0
                && pos < LOOP_TO
                && open.last_pos > duration - LOOP_FROM
                && open.last_pos - pos > LOOP_TO;
            if looped {
                let open = self.open.take()?;
                let mut session = open.log.session.clone();
                let source = open.log.source.clone();
                session.id = new_id();
                self.open = Some(Open::new(session, source, pos, now));
                return Some(open.end(duration, now));
            }
            open.last_pos = pos;
            return None;
        }
        if !paused && let Some(p) = self.pending.take() {
            self.open = Some(Open::new(p.session, p.source, p.start, now));
            if let Some(open) = &mut self.open {
                open.last_pos = pos;
            }
        }
        None
    }

    /// Pause (`PLAYBACK_STOP`) or resume (`PLAYBACK_START`) at the last position.
    pub fn paused(&mut self, paused: bool, now: i64) {
        let Some(open) = &mut self.open else { return };
        if open.paused == paused {
            return;
        }
        open.paused = paused;
        let pos = open.last_pos;
        open.action(!paused, pos, now);
    }

    /// Seek from the last position to `to`: stop there, start again at `to` if playing.
    pub fn seek(&mut self, to: f64, now: i64) {
        let Some(open) = &mut self.open else { return };
        let from = open.last_pos;
        open.action(false, from, now);
        if !open.paused {
            open.action(true, to, now);
        }
        open.last_pos = to;
    }

    /// Natural end of the current stream, nothing preloaded after it.
    pub fn ended(&mut self, duration: f64, now: i64) -> Option<PlayLog> {
        self.pending = None;
        let open = self.open.take()?;
        let end = if duration > 0.0 {
            duration
        } else {
            open.last_pos
        };
        Some(open.end(end, now))
    }

    /// Playback stops (other track loaded, engine stopped, handoff, blind test, closing):
    /// the open session ends at its last position, a pending one is dropped.
    pub fn close(&mut self, now: i64) -> Option<PlayLog> {
        self.pending = None;
        let open = self.open.take()?;
        let pos = open.last_pos;
        Some(open.end(pos, now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal::models::{AudioQuality, TrackId};

    fn session(id: &str, track: u64) -> PlaybackSession {
        PlaybackSession {
            id: id.to_owned(),
            requested: TrackId(track),
            served: TrackId(track),
            presentation: "FULL".to_owned(),
            audio_mode: Some("STEREO".to_owned()),
            quality: AudioQuality::Lossless,
        }
    }

    const SRC: LogSource = LogSource::Album(9);

    /// Positions every second from `from` to `to` (inclusive), clock in ms = 1000 × t.
    fn play(t: &mut PlayLogTracker, from: f64, to: f64, duration: f64, clock: &mut i64) {
        let mut pos = from;
        while pos <= to {
            assert_eq!(
                t.position(pos, duration, false, *clock, || "x".into()),
                None
            );
            pos += 1.0;
            *clock += 1000;
        }
    }

    #[test]
    fn nothing_before_the_first_sound() {
        let mut t = PlayLogTracker::default();
        assert_eq!(t.loaded(session("a", 1), SRC, 0.0, 0), None);
        // Paused right away, then another track: never heard, nothing reported.
        t.position(0.0, 100.0, true, 10, || "x".into());
        assert_eq!(t.loaded(session("b", 2), SRC, 0.0, 20), None);
        assert_eq!(t.close(30), None);
    }

    #[test]
    fn gapless_like_the_sdk_test() {
        let mut t = PlayLogTracker::default();
        let mut clock = 1_000;
        t.loaded(session("a", 1), SRC, 0.0, clock);
        play(&mut t, 0.0, 146.0, 147.0, &mut clock);
        let first = t.advanced(session("b", 2), SRC, 147.0, clock).unwrap();
        assert_eq!(first.session.id, "a");
        assert_eq!(first.start_position, 0.0);
        assert_eq!(first.end_position, 147.0);
        assert_eq!(first.start_timestamp, 1_000);
        play(&mut t, 0.0, 5.0, 200.0, &mut clock);
        let end_of_first = first.end_timestamp;
        let second = t.close(clock).unwrap();
        assert_eq!(second.session.id, "b");
        assert_eq!(second.start_position, 0.0);
        assert_eq!(second.end_position, 5.0);
        assert_eq!(second.start_timestamp, end_of_first);
    }

    #[test]
    fn resumed_track_starts_at_its_position() {
        let mut t = PlayLogTracker::default();
        let mut clock = 0;
        t.loaded(session("a", 1), SRC, 120.0, clock);
        play(&mut t, 120.0, 130.0, 300.0, &mut clock);
        let log = t.loaded(session("b", 2), SRC, 0.0, clock).unwrap();
        assert_eq!(log.start_position, 120.0);
        assert_eq!(log.end_position, 130.0);
    }

    #[test]
    fn pause_and_seek_are_actions() {
        let mut t = PlayLogTracker::default();
        let mut clock = 0;
        t.loaded(session("a", 1), SRC, 0.0, clock);
        play(&mut t, 0.0, 10.0, 200.0, &mut clock);
        t.paused(true, clock);
        t.paused(true, clock + 1); // repeated: ignored
        clock += 5_000;
        t.paused(false, clock);
        t.seek(60.0, clock);
        play(&mut t, 60.0, 70.0, 200.0, &mut clock);
        let log = t.ended(200.0, clock).unwrap();
        let kinds: Vec<(bool, f64)> = log.actions.iter().map(|a| (a.start, a.position)).collect();
        assert_eq!(
            kinds,
            [(false, 10.0), (true, 10.0), (false, 10.0), (true, 60.0)]
        );
        assert_eq!(log.end_position, 200.0);
    }

    #[test]
    fn seek_while_paused_does_not_resume() {
        let mut t = PlayLogTracker::default();
        let mut clock = 0;
        t.loaded(session("a", 1), SRC, 0.0, clock);
        play(&mut t, 0.0, 3.0, 200.0, &mut clock);
        t.paused(true, clock);
        t.seek(50.0, clock);
        let log = t.close(clock).unwrap();
        assert_eq!(log.actions.len(), 2);
        assert!(log.actions.iter().all(|a| !a.start));
        assert_eq!(log.end_position, 50.0);
    }

    #[test]
    fn repeat_one_loop_is_a_new_listen() {
        let mut t = PlayLogTracker::default();
        let mut clock = 0;
        t.loaded(session("a", 1), SRC, 0.0, clock);
        play(&mut t, 0.0, 99.0, 100.0, &mut clock);
        let first = t
            .position(0.2, 100.0, false, clock, || "fresh".into())
            .unwrap();
        assert_eq!(first.session.id, "a");
        assert_eq!(first.end_position, 100.0);
        let second = t.close(clock + 10_000).unwrap();
        assert_eq!(second.session.id, "fresh");
        assert_eq!(second.session.served, TrackId(1));
    }

    #[test]
    fn seek_back_to_the_start_is_not_a_loop() {
        let mut t = PlayLogTracker::default();
        let mut clock = 0;
        t.loaded(session("a", 1), SRC, 0.0, clock);
        play(&mut t, 0.0, 98.0, 100.0, &mut clock);
        t.seek(0.0, clock);
        assert_eq!(t.position(0.5, 100.0, false, clock, || "x".into()), None);
        assert_eq!(t.close(clock).unwrap().session.id, "a");
    }

    #[test]
    fn reloading_after_an_error_ends_the_listen() {
        let mut t = PlayLogTracker::default();
        let mut clock = 0;
        t.loaded(session("a", 1), SRC, 0.0, clock);
        play(&mut t, 0.0, 40.0, 200.0, &mut clock);
        // New URL for the same track, from where it stopped.
        let log = t.loaded(session("b", 1), SRC, 40.0, clock).unwrap();
        assert_eq!(log.end_position, 40.0);
        play(&mut t, 40.0, 50.0, 200.0, &mut clock);
        let log = t.close(clock).unwrap();
        assert_eq!((log.session.id.as_str(), log.start_position), ("b", 40.0));
    }
}
