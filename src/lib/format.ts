// Formatting for display only.

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
import { platform } from "./platform.svelte";

export function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
  const s = Math.floor(seconds);
  const m = Math.floor(s / 60);
  const h = Math.floor(m / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m % 60).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

/** In-memory cache (#453): "track in memory", otherwise the position reached, "up to 2:40",
 *  which only goes up while filling. Not the lead over playback (#498): the cache
 *  advances by segment and the position continuously, the gap would be sawtooth-shaped. */
export function cacheLabel(cached: number, duration: number): string {
  if (!duration || cached <= 0) return "";
  return cached >= duration - 1 ? t.player.cachedWhole : t.player.cachedUntil(formatTime(cached));
}

/** Long duration: "2 h 19 min" or "42 min". */
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
  const parts = [
    p.editorial ? t.format.editorial : p.mine ? t.format.yourPlaylist : t.format.playlist,
  ];
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

/** Cover of the thumbnails (160), falling back to the 320 (fixtures generated before #89). */
export function coverSmall(album: AlbumRef): string | null {
  return album.coverSmallUrl ?? album.coverUrl;
}

/** Large cover (640), same fallback. */
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

export function formatStream(f: Pick<StreamFormat, "codec" | "bitDepth" | "sampleRate">): string {
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

/** mpv's volume changes the amplitude: true except at 100% without mute. */
export function softwareGain(volume: Volume): boolean {
  return volume.muted || volume.level !== 100;
}

/** Readable level for the audio chain ("muted", "60 %"). */
export function volumeLabel(volume: Volume): string {
  return volume.muted ? t.format.muted : t.format.percent(volume.level);
}

/**
 * What the stream becomes up to the output: rate and depth kept or not.
 * `output` is the format actually written to the output (`deviceFormat`, #245), otherwise that
 * of the decoding. "Bit-perfect" is only announced in exclusive mode; in shared mode, Windows
 * still mixes. A volume other than 100% only invalidates that verdict: the other conversions
 * are more important to flag, the volume is then in the detail.
 */
export function signalVerdict(
  stream: StreamFormat | null,
  output: AudioParams | null,
  volume: Volume = { level: 100, muted: false },
  normalization: number | null = null,
  exclusive = false,
  equalizer: string | null = null,
): SignalVerdict {
  const verdict = gainVerdict(stream, output, volume, normalization, exclusive);
  // Equalizer (#428): it modifies the signal like normalization; muting keeps priority.
  if (equalizer === null || volume.muted || (verdict.tone !== "ok" && verdict.tone !== "warn"))
    return verdict;
  const v = t.format.verdict;
  if (streamVerdict(stream, output, exclusive).tone === "ok") {
    let detail = v.equalizerDetail(equalizer, engineName());
    if (normalization !== null) detail = v.withNormalization(detail, gainLabel(normalization));
    if (softwareGain(volume)) detail = v.withVolume(detail, volumeLabel(volume));
    return { label: v.equalizerLabel, tone: "warn", detail };
  }
  return { ...verdict, detail: v.withEqualizer(verdict.detail, equalizer) };
}

/** Verdict of the stream, modified by the software volume and normalization. */
function gainVerdict(
  stream: StreamFormat | null,
  output: AudioParams | null,
  volume: Volume,
  normalization: number | null,
  exclusive: boolean,
): SignalVerdict {
  const verdict = streamVerdict(stream, output, exclusive);
  const v = t.format.verdict;
  // Normalization (#82): gain applied by mpv, like the volume; muting keeps priority.
  if (normalization !== null && !volume.muted && verdict.tone === "ok") {
    const detail = v.normalizedDetail(gainLabel(normalization), engineName());
    return {
      label: v.normalizedLabel,
      tone: "warn",
      detail: softwareGain(volume) ? v.withVolume(detail, volumeLabel(volume)) : detail,
    };
  }
  if (normalization !== null && verdict.tone === "warn") {
    const detail = v.withNormalization(verdict.detail, gainLabel(normalization));
    return {
      ...verdict,
      detail: softwareGain(volume) ? v.withVolume(detail, volumeLabel(volume)) : detail,
    };
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

/** Playback engine shown in the audio chain: mpv on desktop, Media3 on Android. */
export function engineName(): string {
  return platform.mobile ? "Media3" : "mpv";
}

function streamVerdict(
  stream: StreamFormat | null,
  output: AudioParams | null,
  exclusive: boolean,
): SignalVerdict {
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
      detail: exclusive
        ? v.deviceRefused(khz(stream.sampleRate), khz(output.samplerate))
        : `${khz(stream.sampleRate)} → ${khz(output.samplerate)} kHz`,
    };
  }
  if (output.format === "s16" && (stream.bitDepth ?? 16) > 16) {
    return { label: v.reducedLabel, tone: "warn", detail: v.reducedDetail };
  }
  if (exclusive) return { label: v.bitPerfectLabel, tone: "ok", detail: v.bitPerfectDetail };
  return {
    label: v.nativeLabel,
    tone: "ok",
    detail: platform.mobile ? v.nativeDetailAndroid : v.nativeDetail,
  };
}

/** Signed gain in dB: "-6.5 dB", "+2 dB". */
export function gainLabel(db: number): string {
  const abs = num(Math.abs(Math.round(db * 10) / 10));
  return `${db < 0 ? "−" : "+"}${abs} dB`;
}

export function khz(rate: number): string {
  return num(rate / 1000);
}

/** Bitrate in kbps: "5,071 kbps". */
export function kbps(rate: number): string {
  return `${num(rate)} kbps`;
}

/** Role of a credit (#84) in the current language; an unknown role stays as is. */
export function creditRole(role: string): string {
  return t.info.roles[role.toLowerCase().replace(/[^a-z0-9]/g, "")] ?? role;
}

/** Key in the language's notation (Do / C). The API mixes forms (`FSharp`, `Bb`,
 *  `F`): an unknown form is rendered as is. */
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

/** Peak at full scale (1 = 0 dBFS): "0.997 (-0.02 dBFS)". */
export function peak(amplitude: number): string {
  const value = num(amplitude, { maximumFractionDigits: 3 });
  if (amplitude <= 0) return value;
  const dbfs = 20 * Math.log10(amplitude);
  const text = num(Math.abs(dbfs), { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  return `${value} (${dbfs < -0.005 ? "−" : ""}${text} dBFS)`;
}

/** Label of an availability tag; unknown tags are ignored. */
export function tagLabel(tag: string): string | null {
  return (
    {
      LOSSLESS: "Lossless",
      HIRES_LOSSLESS: "Hi-Res",
      DOLBY_ATMOS: "Dolby Atmos",
      SONY_360RA: "360 Reality Audio",
    }[tag] ?? null
  );
}

export function outputFormat(format: string): string {
  const f = t.format;
  return (
    { s16: f.bits(16), s24: f.bits(24), s32: f.bits(32), float: f.float32, offload: f.offload }[
      format
    ] ?? format
  );
}
