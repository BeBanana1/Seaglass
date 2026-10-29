// Mise en forme pour l'affichage uniquement.

import type { AlbumSummary, MediaItem, PlaylistSummary, StreamFormat, Track } from "./ipc";

export function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
  const s = Math.floor(seconds);
  const m = Math.floor(s / 60);
  const h = Math.floor(m / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m % 60).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

/** Durée longue : « 2 h 19 min » ou « 42 min ». */
export function formatLongDuration(seconds: number | null): string | null {
  if (!seconds) return null;
  const minutes = Math.round(seconds / 60);
  const h = Math.floor(minutes / 60);
  return h > 0 ? `${h} h ${minutes % 60} min` : `${minutes} min`;
}

export function plural(n: number, singular: string, pluralForm = `${singular}s`): string {
  return `${n.toLocaleString("fr-FR")} ${n > 1 ? pluralForm : singular}`;
}

export function artistNames(track: Track): string {
  return track.artists.map((a) => a.name).join(", ");
}

export function fullTitle(track: Track): string {
  return track.version ? `${track.title} (${track.version})` : track.title;
}

export function year(date: string | null): string | null {
  return date?.slice(0, 4) ?? null;
}

export function albumKind(album: AlbumSummary): string {
  switch (album.albumType) {
    case "SINGLE":
      return "Single";
    case "EP":
      return "EP";
    default:
      return "Album";
  }
}

export function playlistSubtitle(p: PlaylistSummary): string {
  const parts = [p.editorial ? "Éditoriale" : p.mine ? "Votre playlist" : "Playlist"];
  if (p.numberOfTracks) parts.push(plural(p.numberOfTracks, "titre"));
  return parts.join(" · ");
}

export function mediaTitle(item: MediaItem): string {
  return item.kind === "artist" ? item.name : item.title;
}

export function mediaSubtitle(item: MediaItem): string {
  switch (item.kind) {
    case "album":
      return [item.artists.map((a) => a.name).join(", "), year(item.releaseDate)]
        .filter(Boolean)
        .join(" · ");
    case "artist":
      return "Artiste";
    case "playlist":
      return playlistSubtitle(item);
  }
}

export function mediaImage(item: MediaItem): string | null {
  switch (item.kind) {
    case "album":
      return item.coverUrl;
    case "artist":
      return item.pictureUrl;
    case "playlist":
      return item.imageUrl;
  }
}

export function formatStream(f: StreamFormat): string {
  const codec = f.codec.startsWith("mp4a") ? "AAC" : f.codec.toUpperCase();
  if (f.bitDepth && f.sampleRate && codec === "FLAC") {
    return `FLAC ${f.bitDepth}/${(f.sampleRate / 1000).toLocaleString("fr-FR")}`;
  }
  return codec;
}
