// Courbe de réponse d'un profil d'égaliseur (#428), pour l'affichage seulement : le son
// est filtré par mpv (`mpv/filters.rs`). Biquads du « Audio EQ Cookbook » de R. Bristow-
// Johnson, les mêmes que ceux de libavfilter (`equalizer`, `bass`, `treble` en `t=q`) et
// d'AutoEQ.

import type { EqFilter, EqProfile } from "./ipc";

const FS = 48_000;

/** Préampli appliqué : celui du profil, sinon l'automatique (même règle que
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

/** Gain d'un biquad à la fréquence `hz`, en dB. */
function filterDb(c: number[], hz: number): number {
  const w = (2 * Math.PI * hz) / FS;
  const [c1, s1, c2, s2] = [Math.cos(w), Math.sin(w), Math.cos(2 * w), Math.sin(2 * w)];
  const nr = c[0] + c[1] * c1 + c[2] * c2;
  const ni = -(c[1] * s1 + c[2] * s2);
  const dr = c[3] + c[4] * c1 + c[5] * c2;
  const di = -(c[4] * s1 + c[5] * s2);
  return 10 * Math.log10((nr * nr + ni * ni) / (dr * dr + di * di));
}

/** Réponse du profil (préampli compris) en `points` fréquences réparties en échelle
 *  logarithmique de 20 Hz à 20 kHz : `[hz, dB]`. */
export function response(profile: EqProfile, points = 160): [number, number][] {
  const coeffs = profile.filters.filter((f) => f.gain !== 0).map(coefficients);
  const preamp = effectivePreamp(profile);
  return Array.from({ length: points }, (_, i) => {
    const hz = 20 * 1000 ** (i / (points - 1));
    return [hz, coeffs.reduce((db, c) => db + filterDb(c, hz), preamp)];
  });
}
