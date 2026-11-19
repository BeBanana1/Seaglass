import { describe, expect, it } from "vitest";
import { clockPosition, clockSample, type PositionClock } from "./positionClock";

const playing = (pos: number, key: unknown = 1) => ({ pos, running: true, key });

/** Samples every 250 ms with the given positions, and the shown position sampled every 10 ms. */
function shownTrace(positions: number[]): number[] {
  let clock: PositionClock | null = null;
  const trace: number[] = [];
  positions.forEach((pos, i) => {
    clock = clockSample(clock, playing(pos), i * 250);
    for (let t = 0; t < 250; t += 10) trace.push(clockPosition(clock, i * 250 + t));
  });
  return trace;
}

describe("position clock", () => {
  it("advances with time while running", () => {
    const c = clockSample(null, playing(10), 1000);
    expect(clockPosition(c, 1500)).toBeCloseTo(10.5);
  });

  it("stays put while paused", () => {
    const c = clockSample(null, { pos: 10, running: false, key: 1 }, 1000);
    expect(clockPosition(c, 5000)).toBe(10);
  });

  it("never goes backwards on a time earlier than the sample", () => {
    const c = clockSample(null, playing(10), 1000);
    expect(clockPosition(c, 990)).toBe(10);
  });

  it("does not step back when a sample lags behind the display", () => {
    // Jitter of ±80 ms around a regular clock.
    const trace = shownTrace([0, 0.33, 0.42, 0.83, 0.92, 1.33, 1.42, 1.83]);
    for (let i = 1; i < trace.length; i++) expect(trace[i]).toBeGreaterThanOrEqual(trace[i - 1]);
  });

  it("converges to the reported position", () => {
    let c = clockSample(null, playing(0), 0);
    c = clockSample(c, playing(0.15), 250); // display was 0.25 ahead
    expect(clockPosition(c, 250)).toBeCloseTo(0.25);
    expect(clockPosition(c, 1250)).toBeCloseTo(1.15);
  });

  it("snaps on a seek", () => {
    let c = clockSample(null, playing(30), 0);
    c = clockSample(c, playing(15), 250);
    expect(clockPosition(c, 250)).toBe(15);
  });

  it("snaps on a new track or after a pause", () => {
    let c = clockSample(null, playing(10, 1), 0);
    c = clockSample(c, playing(0.1, 2), 250);
    expect(clockPosition(c, 250)).toBe(0.1);
    c = clockSample(c, { pos: 0.4, running: false, key: 2 }, 500);
    c = clockSample(c, playing(0.4, 2), 2000);
    expect(clockPosition(c, 2000)).toBe(0.4);
  });
});
