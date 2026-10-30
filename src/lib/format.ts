// Mise en forme pour l'affichage uniquement.

import type {
  AlbumSummary,
  AudioParams,
  MediaItem,
  PlaylistSummary,
  StreamFormat,
  Track,
  Volume,
} from "./ipc";

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
    case "mix":
      return item.subtitle ?? "Mix";
  }
}

export function mediaImage(item: MediaItem): string | null {
  switch (item.kind) {
    case "album":
      return item.coverUrl;
    case "artist":
      return item.pictureUrl;
    case "playlist":
    case "mix":
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

export type SignalTone = "ok" | "warn" | "lossy" | "idle";

export interface SignalVerdict {
  label: string;
  tone: SignalTone;
  detail: string;
}

/** Le volume de mpv modifie l'amplitude : vrai sauf à 100 % sans coupure. */
export function softwareGain(volume: Volume): boolean {
  return volume.muted || volume.level !== 100;
}

/** Niveau lisible pour la chaîne audio (« coupé », « 60 % »). */
export function volumeLabel(volume: Volume): string {
  return volume.muted ? "coupé" : `${volume.level} %`;
}

/**
 * Ce que mpv fait du flux reçu : fréquence et profondeur conservées ou non. Un volume
 * différent de 100 % n'invalide que le verdict « Fréquence native » : les autres
 * conversions restent plus importantes à signaler, le volume est alors dans le détail.
 */
export function signalVerdict(
  stream: StreamFormat | null,
  output: AudioParams | null,
  volume: Volume = { level: 100, muted: false },
): SignalVerdict {
  const verdict = streamVerdict(stream, output);
  if (verdict.tone === "ok" && softwareGain(volume)) {
    return {
      label: volume.muted ? "Son coupé" : "Volume logiciel",
      tone: "warn",
      detail: volume.muted
        ? "son coupé dans mpv"
        : `volume ${volume.level} % : gain appliqué par mpv, plus bit-perfect (100 % pour le rétablir)`,
    };
  }
  if (verdict.tone === "warn" && softwareGain(volume)) {
    return { ...verdict, detail: `${verdict.detail} ; volume ${volumeLabel(volume)}` };
  }
  return verdict;
}

function streamVerdict(stream: StreamFormat | null, output: AudioParams | null): SignalVerdict {
  if (!stream) return { label: "Aucun flux", tone: "idle", detail: "" };
  if (stream.codec !== "flac") {
    return { label: "Compressé", tone: "lossy", detail: "flux AAC avec perte" };
  }
  if (!output) return { label: "En attente", tone: "idle", detail: "sortie audio pas encore ouverte" };
  if (stream.sampleRate && output.samplerate !== stream.sampleRate) {
    return {
      label: "Rééchantillonné",
      tone: "warn",
      detail: `${khz(stream.sampleRate)} → ${khz(output.samplerate)} kHz`,
    };
  }
  if (output.format === "s16" && (stream.bitDepth ?? 16) > 16) {
    return { label: "Profondeur réduite", tone: "warn", detail: "24 → 16 bits" };
  }
  return {
    label: "Fréquence native",
    tone: "ok",
    detail: "mpv transmet sans conversion ; en mode partagé, Windows peut encore mixer",
  };
}

export function khz(rate: number): string {
  return (rate / 1000).toLocaleString("fr-FR");
}

export function outputFormat(format: string): string {
  return { s16: "16 bits", s24: "24 bits", s32: "32 bits", float: "32 bits flottant" }[format] ?? format;
}
