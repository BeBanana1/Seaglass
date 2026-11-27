import { describe, expect, it } from "vitest";
import {
  countLoops,
  LOOPS_FOR_INFINITY,
  NO_LOOPS,
  parsecs,
  SEQUENCE,
  sequenceStep,
  stationClock,
  type Loops,
} from "./playback";

/** Plays successive positions (duration 200 s) and returns the count. */
function play(positions: number[], repeatOne = true, id = 7, start: Loops = NO_LOOPS): Loops {
  return positions.reduce((loops, pos) => countLoops(loops, id, pos, 200, repeatOne), start);
}

describe("countLoops", () => {
  it("counts a return to the start after the end", () => {
    expect(play([0, 100, 198, 0.5]).count).toBe(1);
    expect(play([0, 197, 1, 100, 199, 0.2]).count).toBe(2);
  });

  it("ignores a return to the start in the middle of a listen", () => {
    expect(play([0, 100, 0.5]).count).toBe(0);
  });

  it("resets outside Repeat One or on another track", () => {
    const looped = play([0, 199, 0.5]);
    expect(countLoops(looped, 8, 0, 200, true).count).toBe(0);
    expect(countLoops(looped, 7, 0, 200, false).count).toBe(0);
  });

  it("reaches infinity after ten listens", () => {
    const positions = Array.from({ length: LOOPS_FOR_INFINITY }, () => [199, 0.4]).flat();
    expect(play([0, ...positions]).count).toBe(LOOPS_FOR_INFINITY);
  });
});

describe("sequenceStep", () => {
  const run = (keys: string[]) => keys.reduce(sequenceStep, 0);

  it("recognizes the full sequence, B and A in capitals too", () => {
    expect(run(SEQUENCE)).toBe(SEQUENCE.length);
    expect(run(SEQUENCE.map((k) => (k.length === 1 ? k.toUpperCase() : k)))).toBe(SEQUENCE.length);
  });

  it("restarts on a wrong key", () => {
    expect(run(["ArrowUp", "ArrowUp", "x"])).toBe(0);
    expect(run(["ArrowUp", "ArrowDown"])).toBe(0);
  });

  it("an extra ↑ does not break the start", () => {
    expect(run(["ArrowUp", "ArrowUp", "ArrowUp", ...SEQUENCE.slice(2)])).toBe(SEQUENCE.length);
  });
});

describe("stationClock", () => {
  it("does the minute in 58.5 s then stops", () => {
    expect(stationClock(0, 300)).toBe(0);
    expect(stationClock(29.25, 300)).toBeCloseTo(30);
    expect(stationClock(58.5, 300)).toBeCloseTo(60);
    expect(stationClock(59.5, 300)).toBe(60);
  });

  it("restarts at the full minute", () => {
    expect(stationClock(60, 300)).toBe(60);
    expect(stationClock(61, 300)).toBeGreaterThan(61);
  });

  it("never exceeds the duration", () => {
    expect(stationClock(119, 119.5)).toBe(119.5);
  });
});

describe("parsecs", () => {
  it("a minute is worth a parsec, to the tenth", () => {
    expect(parsecs(60)).toBe(1);
    expect(parsecs(213)).toBe(3.6);
  });
});
