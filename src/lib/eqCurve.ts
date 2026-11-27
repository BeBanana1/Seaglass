// Response curve of an equalizer profile (#428), for display only: the sound
// is filtered by mpv (`mpv/filters.rs`). Biquads from R. Bristow-Johnson's "Audio EQ
// Cookbook", the same as libavfilter's (`equalizer`, `bass`, `treble` with `t=q`) and
// AutoEQ's.

import type { EqFilter, EqProfile } from "./ipc";

const FS = 48_000;

/** Preamp applied: the profile's, otherwise the automatic one (same rule as
 *  `EqProfile::effective_preamp`). */
export function effectivePreamp(profile: EqProfile): number {
  return profile.preamp ?? -Math.max(0, ...profile.filters.map((f) => f.gain));
}

/** Coefficients `[b0, b1, b2, a0, a1, a2]` d'un filtre. */
function coefficients(f: EqFilter): number[] {
  const a = 10 ** (f.gain / 40);
  const w0 = (2 * Math.PI * f.freq) / FS;
  const cos = Math.cos(w0);
  const alpha = Math.sin(w0) / (2 * f.q);
  if (f.kind === "peaking") {
    return [1 + alpha * a, -2 * cos, 1 - alpha * a, 1 + alpha / a, -2 * cos, 1 - alpha / a];
  }
  const s = 2 * Math.sqrt(a) * alpha;
  if (f.kind === "lowShelf") {
    return [
      a * (a + 1 - (a - 1) * cos + s),
      2 * a * (a - 1 - (a + 1) * cos),
      a * (a + 1 - (a - 1) * cos - s),
      a + 1 + (a - 1) * cos + s,
      -2 * (a - 1 + (a + 1) * cos),
      a + 1 + (a - 1) * cos - s,
    ];
  }
  return [
    a * (a + 1 + (a - 1) * cos + s),
    -2 * a * (a - 1 + (a + 1) * cos),
    a * (a + 1 + (a - 1) * cos - s),
    a + 1 - (a - 1) * cos + s,
    2 * (a - 1 - (a + 1) * cos),
    a + 1 - (a - 1) * cos - s,
  ];
}

/** Gain of a biquad at frequency `hz`, in dB. */
function filterDb(c: number[], hz: number): number {
  const w = (2 * Math.PI * hz) / FS;
  const [c1, s1, c2, s2] = [Math.cos(w), Math.sin(w), Math.cos(2 * w), Math.sin(2 * w)];
  const nr = c[0] + c[1] * c1 + c[2] * c2;
  const ni = -(c[1] * s1 + c[2] * s2);
  const dr = c[3] + c[4] * c1 + c[5] * c2;
  const di = -(c[4] * s1 + c[5] * s2);
  return 10 * Math.log10((nr * nr + ni * ni) / (dr * dr + di * di));
}

/** Response of the profile (preamp included) over `points` frequencies spread on a
 *  logarithmic scale from 20 Hz to 20 kHz: `[hz, dB]`. */
export function response(profile: EqProfile, points = 160): [number, number][] {
  const coeffs = profile.filters.filter((f) => f.gain !== 0).map(coefficients);
  const preamp = effectivePreamp(profile);
  return Array.from({ length: points }, (_, i) => {
    const hz = 20 * 1000 ** (i / (points - 1));
    return [hz, coeffs.reduce((db, c) => db + filterDb(c, hz), preamp)];
  });
}
