// Easter eggs tied to playback and the keyboard (#399, batch B): pure logic, no DOM.

import type { TrackId } from "../ipc";

/** Loops of the current track on Repeat One: `last` is the last position seen. */
export interface Loops {
  id: TrackId | null;
  count: number;
  last: number;
}

export const NO_LOOPS: Loops = { id: null, count: 0, last: 0 };

/** After 9 returns to the start, the track has played 10 times in a row: the icon turns to ∞. */
export const LOOPS_FOR_INFINITY = 9;

/** Counts a lap when the position restarts from the start after passing near the end, on
 *  the same track on Repeat One. Another track or another mode resets the count; a
 *  return to the start in the middle of a listen (Previous, seek) does not count. */
export function countLoops(
  prev: Loops,
  id: TrackId | null,
  pos: number,
  duration: number,
  repeatOne: boolean,
): Loops {
  if (!repeatOne || id === null || id !== prev.id) return { id, count: 0, last: pos };
  const wrapped = duration > 0 && prev.last >= duration - 5 && pos < 3;
  return { id, count: wrapped ? prev.count + 1 : prev.count, last: pos };
}

/** Keyboard sequence (↑↑↓↓←→←→BA), keys of `KeyboardEvent.key`. */
export const SEQUENCE = [
  "ArrowUp",
  "ArrowUp",
  "ArrowDown",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "ArrowLeft",
  "ArrowRight",
  "b",
  "a",
];

/** Advances in the sequence: returns the number of correct keys, `SEQUENCE.length` when
 *  it is complete. A wrong key restarts from zero, or from one if it is a start. */
export function sequenceStep(progress: number, key: string): number {
  const k = key.length === 1 ? key.toLowerCase() : key;
  if (k === SEQUENCE[progress]) return progress + 1;
  return k === SEQUENCE[0] ? (progress === 2 ? 2 : 1) : 0;
}

/** Lap of a station clock's dial: the second hand goes round in 58.5 s. */
const STATION_LAP = 58.5;

/** August 1st: position shown by the progress bar, like the second hand of a station
 *  clock. Each minute is covered in 58.5 s, the bar stops on the full minute,
 *  then restarts when the real minute has passed. Never beyond `duration`. */
export function stationClock(pos: number, duration: number): number {
  if (pos <= 0) return 0;
  const minute = Math.floor(pos / 60) * 60;
  const shown = minute + Math.min(((pos - minute) * 60) / STATION_LAP, 60);
  return duration > 0 ? Math.min(shown, duration) : shown;
}

/** "kessel" search: a duration in parsecs, a whimsical unit (one minute is worth a
 *  parsec), rounded to the tenth. */
export function parsecs(seconds: number): number {
  return Math.round((seconds / 60) * 10) / 10;
}
