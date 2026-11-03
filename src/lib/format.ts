// Mise en forme pour l'affichage uniquement.

import type {
  AlbumRef,
  AlbumSummary,
  AudioParams,
  MediaItem,
  PlaylistSummary,
  StreamFormat,
  Track,
  Volume,
} from "./ipc";
import { num, t } from "./i18n.svelte";

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
  return t.format.longDuration(Math.floor(minutes / 60), minutes % 60);
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
      return t.format.single;
    case "EP":
      return t.format.ep;
    default:
      return t.format.album;
  }
}

export function playlistSubtitle(p: PlaylistSummary): string {
  const parts = [p.editorial ? t.format.editorial : p.mine ? t.format.yourPlaylist : t.format.playlist];
  if (p.numberOfTracks) parts.push(t.common.tracks(p.numberOfTracks));
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
      return t.format.artist;
    case "playlist":
      return playlistSubtitle(item);
    case "mix":
      return item.subtitle ?? t.format.mix;
  }
}

/** Pochette des vignettes (160), repli sur la 320 (fixtures générées avant #89). */
export function coverSmall(album: AlbumRef): string | null {
  return album.coverSmallUrl ?? album.coverUrl;
}

/** Grande pochette (640), même repli. */
export function coverLarge(album: AlbumRef | AlbumSummary): string | null {
  return album.coverLargeUrl ?? album.coverUrl;
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
    return `FLAC ${f.bitDepth}/${khz(f.sampleRate)}`;
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
  return volume.muted ? t.format.muted : t.format.percent(volume.level);
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
  normalization: number | null = null,
): SignalVerdict {
  const verdict = streamVerdict(stream, output);
  const v = t.format.verdict;
  // Normalisation (#82) : gain appliqué par mpv, comme le volume ; la coupure reste prioritaire.
  if (normalization !== null && !volume.muted && verdict.tone === "ok") {
    const detail = v.normalizedDetail(gainLabel(normalization));
    return {
      label: v.normalizedLabel,
      tone: "warn",
      detail: softwareGain(volume) ? v.withVolume(detail, volumeLabel(volume)) : detail,
    };
  }
  if (normalization !== null && verdict.tone === "warn") {
    const detail = v.withNormalization(verdict.detail, gainLabel(normalization));
    return { ...verdict, detail: softwareGain(volume) ? v.withVolume(detail, volumeLabel(volume)) : detail };
  }
  if (verdict.tone === "ok" && softwareGain(volume)) {
    return {
      label: volume.muted ? v.mutedLabel : v.gainLabel,
      tone: "warn",
      detail: volume.muted ? v.mutedDetail : v.gainDetail(volume.level),
    };
  }
  if (verdict.tone === "warn" && softwareGain(volume)) {
    return { ...verdict, detail: v.withVolume(verdict.detail, volumeLabel(volume)) };
  }
  return verdict;
}

function streamVerdict(stream: StreamFormat | null, output: AudioParams | null): SignalVerdict {
  const v = t.format.verdict;
  if (!stream) return { label: v.noStream, tone: "idle", detail: "" };
  if (stream.codec !== "flac") {
    return { label: v.lossyLabel, tone: "lossy", detail: v.lossyDetail };
  }
  if (!output) return { label: v.waitingLabel, tone: "idle", detail: v.waitingDetail };
  if (stream.sampleRate && output.samplerate !== stream.sampleRate) {
    return {
      label: v.resampledLabel,
      tone: "warn",
      detail: `${khz(stream.sampleRate)} → ${khz(output.samplerate)} kHz`,
    };
  }
  if (output.format === "s16" && (stream.bitDepth ?? 16) > 16) {
    return { label: v.reducedLabel, tone: "warn", detail: v.reducedDetail };
  }
  return { label: v.nativeLabel, tone: "ok", detail: v.nativeDetail };
}

/** Gain en dB signé : « −6,5 dB », « +2 dB ». */
export function gainLabel(db: number): string {
  const abs = num(Math.abs(Math.round(db * 10) / 10));
  return `${db < 0 ? "−" : "+"}${abs} dB`;
}

export function khz(rate: number): string {
  return num(rate / 1000);
}

/** Débit en kbps : « 5 071 kbps ». */
export function kbps(rate: number): string {
  return `${num(rate)} kbps`;
}

/** Tonalité dans la notation de la langue (Do / C). L'API mêle les formes (`FSharp`, `Bb`,
 *  `F`) : une forme inconnue est rendue telle quelle. */
export function musicalKey(key: string | null, scale: string | null): string | null {
  if (!key) return null;
  const m = /^([A-G])(Sharp|#|Flat|b)?$/.exec(key);
  if (!m) return [key, scale?.toLowerCase()].filter(Boolean).join(" ");
  const accidental = !m[2] ? "" : m[2] === "Sharp" || m[2] === "#" ? "♯" : "♭";
  const mode = scale === "MAJOR" ? t.format.major : scale === "MINOR" ? t.format.minor : "";
  return `${t.format.notes[m[1]]}${accidental}${mode}`;
}

/** Gain ReplayGain : « −6,17 dB ». */
export function decibels(gain: number): string {
  const text = num(Math.abs(gain), { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  return `${gain < 0 ? "−" : gain > 0 ? "+" : ""}${text} dB`;
}

/** Crête en pleine échelle (1 = 0 dBFS) : « 0,997 (−0,02 dBFS) ». */
export function peak(amplitude: number): string {
  const value = num(amplitude, { maximumFractionDigits: 3 });
  if (amplitude <= 0) return value;
  const dbfs = 20 * Math.log10(amplitude);
  const text = num(Math.abs(dbfs), { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  return `${value} (${dbfs < -0.005 ? "−" : ""}${text} dBFS)`;
}

/** Libellé d'un tag de disponibilité ; les tags inconnus sont ignorés. */
export function tagLabel(tag: string): string | null {
  return (
    { LOSSLESS: "Lossless", HIRES_LOSSLESS: "Hi-Res", DOLBY_ATMOS: "Dolby Atmos", SONY_360RA: "360 Reality Audio" }[
      tag
    ] ?? null
  );
}

export function outputFormat(format: string): string {
  const f = t.format;
  return { s16: f.bits(16), s24: f.bits(24), s32: f.bits(32), float: f.float32 }[format] ?? format;
}
