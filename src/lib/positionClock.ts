// Displayed playback position between two `player-state` events (~4 Hz, #501).
//
// mpv's `time_pos` has a few tens of ms of jitter (IPC, audio output buffering), so snapping
// the display to every sample makes a fast-moving bar step back four times a second. During
// continuous playback of the same track, the clock keeps the shown position and absorbs the gap
// over `CATCH_UP_SECONDS` by running slightly faster or slower. It snaps only on a real
// discontinuity: pause, loading, track change, seek or a gap of `SNAP_SECONDS` or more.

/** A gap this large is a seek or a replay, not jitter: the display jumps. */
export const SNAP_SECONDS = 1;
/** Time over which a smaller gap is absorbed. Not shorter than `SNAP_SECONDS`, so the speed
 *  stays between 0 and 2 and the position never goes backwards. */
export const CATCH_UP_SECONDS = 1;

export interface PositionSample {
  /** Position reported by the backend, in seconds. */
  pos: number;
  /** Position advancing (not paused, loading, buffering or stopped). */
  running: boolean;
  /** Identifies the playback (track id): a new one always snaps. */
  key: unknown;
}

export interface PositionClock {
  /** Shown position at `at`. */
  base: number;
  /** `performance.now()` of the sample, in ms. */
  at: number;
  /** Gap to absorb, in seconds (positive: the display is behind). */
  gap: number;
  running: boolean;
  key: unknown;
}

/** Shown position at `now` (ms, same clock as `at`). */
export function clockPosition(c: PositionClock, now: number): number {
  if (!c.running) return c.base;
  const elapsed = Math.max(0, (now - c.at) / 1000);
  return c.base + elapsed + c.gap * Math.min(1, elapsed / CATCH_UP_SECONDS);
}

/** New clock after receiving `s` at `now`. */
export function clockSample(
  prev: PositionClock | null,
  s: PositionSample,
  now: number,
): PositionClock {
  if (prev?.running && s.running && prev.key === s.key) {
    const shown = clockPosition(prev, now);
    const gap = s.pos - shown;
    if (Math.abs(gap) < SNAP_SECONDS)
      return { base: shown, at: now, gap, running: true, key: s.key };
  }
  return { base: s.pos, at: now, gap: 0, running: s.running, key: s.key };
}
