// Mise en forme pour l'affichage uniquement.

import type { StreamFormat, Track } from "./ipc";

export function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
  const s = Math.floor(seconds);
  const m = Math.floor(s / 60);
  const h = Math.floor(m / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m % 60).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

export function artistNames(track: Track): string {
  return track.artists.map((a) => a.name).join(", ");
}

export function fullTitle(track: Track): string {
  return track.version ? `${track.title} (${track.version})` : track.title;
}

export function formatStream(f: StreamFormat): string {
  const codec = f.codec.startsWith("mp4a") ? "AAC" : f.codec.toUpperCase();
  if (f.bitDepth && f.sampleRate && codec === "FLAC") {
    return `FLAC ${f.bitDepth}/${(f.sampleRate / 1000).toLocaleString("fr-FR")}`;
  }
  return codec;
}
