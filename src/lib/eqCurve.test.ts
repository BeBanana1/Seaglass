import { describe, expect, it } from "vitest";
import { effectivePreamp, response } from "./eqCurve";
import type { EqProfile } from "./ipc";

const at = (curve: [number, number][], hz: number) =>
  curve.reduce((best, p) => (Math.abs(p[0] - hz) < Math.abs(best[0] - hz) ? p : best))[1];

describe("eqCurve", () => {
  it("takes the profile's preamp, otherwise compensates the strongest gain", () => {
    const p: EqProfile = {
      id: 1,
      name: "x",
      preamp: null,
      filters: [
        { kind: "peaking", freq: 1000, gain: 4, q: 1 },
        { kind: "peaking", freq: 3000, gain: -6, q: 1 },
      ],
    };
    expect(effectivePreamp(p)).toBe(-4);
    expect(effectivePreamp({ ...p, preamp: -6.2 })).toBe(-6.2);
    expect(effectivePreamp({ ...p, filters: [] })).toBe(-0);
  });

  it("gives the peak's gain at its frequency and nothing far from it", () => {
    const curve = response(
      { id: 1, name: "x", preamp: 0, filters: [{ kind: "peaking", freq: 1000, gain: 6, q: 1.41 }] },
      400,
    );
    expect(at(curve, 1000)).toBeCloseTo(6, 0);
    expect(Math.abs(at(curve, 20))).toBeLessThan(0.1);
  });

  it("raises the bass with a low shelf, preamp included", () => {
    const curve = response({
      id: 1,
      name: "x",
      preamp: -5,
      filters: [{ kind: "lowShelf", freq: 105, gain: 5, q: 0.7 }],
    });
    expect(at(curve, 20)).toBeCloseTo(0, 0);
    expect(at(curve, 10_000)).toBeCloseTo(-5, 0);
  });

  it("lowers the treble with a high shelf", () => {
    const curve = response({
      id: 1,
      name: "x",
      preamp: 0,
      filters: [{ kind: "highShelf", freq: 5000, gain: -4, q: 0.7 }],
    });
    expect(at(curve, 20_000)).toBeLessThan(-3);
    expect(Math.abs(at(curve, 100))).toBeLessThan(0.1);
  });
});
