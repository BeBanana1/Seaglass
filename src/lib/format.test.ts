import { afterEach, describe, expect, it } from "vitest";
import {
  albumKind,
  cacheLabel,
  creditRole,
  decibels,
  formatLongDuration,
  formatStream,
  formatTime,
  fullTitle,
  gainLabel,
  kbps,
  musicalKey,
  outputFormat,
  peak,
  playlistSubtitle,
  signalVerdict,
  tagLabel,
  year,
} from "./format";
import { i18n } from "./i18n.svelte";
import type { AlbumSummary, AudioParams, PlaylistSummary, StreamFormat } from "./ipc";
import { platform } from "./platform.svelte";
import { entry } from "./testing/tracks";

// Espaces insécables de `Intl` (milliers en français) ramenées à des espaces simples.
const plain = (s: string) => s.replace(/[  ]/g, " ");

afterEach(() => {
  i18n.lang = "fr";
  platform.mobile = false;
});

describe("cacheLabel", () => {
  it("donne la position atteinte, pas l'avance sur la lecture (#498)", () => {
    expect(cacheLabel(160.7, 300)).toBe("en mémoire jusqu'à 2:40");
    i18n.lang = "en";
    expect(cacheLabel(160.7, 300)).toBe("in memory up to 2:40");
  });

  it("annonce le titre entier à moins d'une seconde de la fin", () => {
    expect(cacheLabel(299.2, 300)).toBe("titre en mémoire");
  });

  it("ne dit rien sans cache ni durée", () => {
    expect(cacheLabel(0, 300)).toBe("");
    expect(cacheLabel(40, 0)).toBe("");
  });
});

describe("formatTime", () => {
  it("écrit minutes et heures", () => {
    expect(formatTime(0)).toBe("0:00");
    expect(formatTime(65.9)).toBe("1:05");
    expect(formatTime(3600 + 2 * 60 + 3)).toBe("1:02:03");
  });

  it("rend 0:00 pour une valeur absente ou fausse", () => {
    expect(formatTime(-1)).toBe("0:00");
    expect(formatTime(Number.NaN)).toBe("0:00");
    expect(formatTime(Number.POSITIVE_INFINITY)).toBe("0:00");
  });
});

describe("formatLongDuration", () => {
  it("arrondit à la minute", () => {
    expect(formatLongDuration(42 * 60 + 20)).toBe("42 min");
    expect(formatLongDuration(2 * 3600 + 19 * 60 + 40)).toBe("2 h 20 min");
  });

  it("ne rend rien sans durée", () => {
    expect(formatLongDuration(null)).toBeNull();
    expect(formatLongDuration(0)).toBeNull();
  });
});

describe("titres et sous-titres", () => {
  it("ajoute la version entre parenthèses", () => {
    expect(fullTitle(entry({ title: "Halo", version: "Live" }))).toBe("Halo (Live)");
    expect(fullTitle(entry({ title: "Halo" }))).toBe("Halo");
  });

  it("garde l'année d'une date", () => {
    expect(year("2008-11-12")).toBe("2008");
    expect(year(null)).toBeNull();
  });

  it("nomme le type d'album", () => {
    const album = (albumType: string) => ({ albumType }) as unknown as AlbumSummary;
    expect(albumKind(album("SINGLE"))).toBe("Single");
    expect(albumKind(album("ALBUM"))).toBe("Album");
    expect(albumKind(album("COMPILATION"))).toBe("Album");
  });

  it("décrit une playlist dans la langue courante", () => {
    const p = (fields: Partial<PlaylistSummary>) =>
      ({ editorial: false, mine: false, numberOfTracks: null, ...fields }) as PlaylistSummary;
    expect(playlistSubtitle(p({ editorial: true, mine: true }))).toBe("Éditoriale");
    expect(playlistSubtitle(p({ mine: true, numberOfTracks: 12 }))).toBe("Votre playlist · 12 titres");
    i18n.lang = "en";
    expect(playlistSubtitle(p({ numberOfTracks: 1 }))).toBe("Playlist · 1 track");
  });
});

describe("formatStream", () => {
  const stream = (fields: Partial<StreamFormat>): StreamFormat => ({
    quality: "LOSSLESS",
    codec: "flac",
    bitDepth: null,
    sampleRate: null,
    bitrate: null,
    hiresEdition: null,
    ...fields,
  });

  it("donne profondeur et fréquence d'un FLAC, séparateur décimal de la langue", () => {
    expect(formatStream(stream({ bitDepth: 24, sampleRate: 96000 }))).toBe("FLAC 24/96");
    expect(formatStream(stream({ bitDepth: 16, sampleRate: 44100 }))).toBe("FLAC 16/44,1");
    i18n.lang = "en";
    expect(formatStream(stream({ bitDepth: 16, sampleRate: 44100 }))).toBe("FLAC 16/44.1");
  });

  it("nomme les autres codecs", () => {
    expect(formatStream(stream({ codec: "mp4a.40.2" }))).toBe("AAC");
    expect(formatStream(stream({}))).toBe("FLAC");
  });
});

describe("signalVerdict", () => {
  const hires: StreamFormat = {
    quality: "HI_RES_LOSSLESS",
    codec: "flac",
    bitDepth: 24,
    sampleRate: 96000,
    bitrate: null,
    hiresEdition: null,
  };
  const out = (samplerate: number, format = "s32"): AudioParams => ({ format, samplerate, channelCount: 2 });

  it("sans flux ou en attente de la sortie", () => {
    expect(signalVerdict(null, null).tone).toBe("idle");
    expect(signalVerdict(hires, null)).toMatchObject({ tone: "idle", label: "En attente" });
  });

  it("signale un flux compressé", () => {
    expect(signalVerdict({ ...hires, codec: "mp4a.40.2" }, out(44100)).tone).toBe("lossy");
  });

  it("reconnaît la fréquence native, avec le détail propre à Android", () => {
    expect(signalVerdict(hires, out(96000))).toMatchObject({ tone: "ok", label: "Fréquence native" });
    platform.mobile = true;
    expect(signalVerdict(hires, out(96000)).detail).toMatch(/^Media3/);
  });

  it("signale un rééchantillonnage et une profondeur réduite", () => {
    expect(signalVerdict(hires, out(48000))).toMatchObject({ tone: "warn", detail: "96 → 48 kHz" });
    expect(signalVerdict(hires, out(96000, "s16"))).toMatchObject({ tone: "warn", label: "Profondeur réduite" });
    // Un 16 bits en sortie 16 bits reste natif.
    expect(signalVerdict({ ...hires, bitDepth: 16 }, out(96000, "s16")).tone).toBe("ok");
  });

  it("retire le verdict natif dès que le volume n'est pas à 100 %", () => {
    expect(signalVerdict(hires, out(96000), { level: 60, muted: false })).toMatchObject({
      tone: "warn",
      label: "Volume logiciel",
    });
    expect(signalVerdict(hires, out(96000), { level: 100, muted: true })).toMatchObject({ label: "Son coupé" });
  });

  it("garde une conversion prioritaire sur le volume, qui passe dans le détail", () => {
    const v = signalVerdict(hires, out(48000), { level: 60, muted: false });
    expect(v.label).toBe("Rééchantillonné");
    expect(plain(v.detail)).toBe("96 → 48 kHz ; volume 60 %");
  });

  it("signale la normalisation, sauf son coupé", () => {
    const v = signalVerdict(hires, out(96000), { level: 100, muted: false }, -6.5);
    expect(v).toMatchObject({ tone: "warn", label: "Volume normalisé" });
    expect(v.detail).toBe("ReplayGain −6,5 dB appliqué par mpv, plus bit-perfect");
    expect(signalVerdict(hires, out(96000), { level: 100, muted: true }, -6.5).label).toBe("Son coupé");
    expect(signalVerdict(hires, out(48000), undefined, 2).detail).toBe("96 → 48 kHz ; ReplayGain +2 dB");
  });

  it("annonce le bit-perfect seulement en mode exclusif (#245)", () => {
    const full = { level: 100, muted: false };
    expect(signalVerdict(hires, out(96000), full, null, true)).toMatchObject({ tone: "ok", label: "Bit-perfect" });
    // Fréquence refusée par le périphérique : mpv rééchantillonne, la cause est donnée.
    expect(plain(signalVerdict(hires, out(48000), full, null, true).detail)).toBe(
      "96 → 48 kHz : fréquence refusée par le périphérique en exclusif",
    );
    // Volume logiciel gardé en exclusif, mais signalé.
    expect(signalVerdict(hires, out(96000), { level: 60, muted: false }, null, true)).toMatchObject({
      tone: "warn",
      label: "Volume logiciel",
    });
  });

  it("signale l'égaliseur, en exclusif aussi, sauf son coupé (#428)", () => {
    const full = { level: 100, muted: false };
    const v = signalVerdict(hires, out(96000), full, null, true, "HD 600");
    expect(v).toMatchObject({ tone: "warn", label: "Égaliseur" });
    expect(v.detail).toBe("profil « HD 600 » appliqué par mpv, plus bit-perfect");
    expect(plain(signalVerdict(hires, out(96000), { level: 60, muted: false }, -6.5, false, "HD 600").detail)).toBe(
      "profil « HD 600 » appliqué par mpv, plus bit-perfect ; ReplayGain −6,5 dB ; volume 60 %",
    );
    expect(signalVerdict(hires, out(96000), { level: 100, muted: true }, null, false, "HD 600").label).toBe("Son coupé");
    expect(plain(signalVerdict(hires, out(48000), full, null, false, "HD 600").detail)).toBe(
      "96 → 48 kHz ; égaliseur « HD 600 »",
    );
    expect(signalVerdict({ ...hires, codec: "mp4a.40.2" }, out(96000), full, null, false, "HD 600").tone).toBe("lossy");
  });
});

describe("nombres", () => {
  it("écrit un gain signé avec un vrai signe moins", () => {
    expect(gainLabel(-6.54)).toBe("−6,5 dB");
    expect(gainLabel(2)).toBe("+2 dB");
    expect(decibels(-6.17)).toBe("−6,17 dB");
    expect(decibels(0)).toBe("0,00 dB");
    i18n.lang = "en";
    expect(decibels(1.5)).toBe("+1.50 dB");
  });

  it("écrit une crête en dBFS", () => {
    expect(peak(0.997)).toBe("0,997 (−0,03 dBFS)");
    expect(peak(1)).toBe("1 (0,00 dBFS)");
    expect(peak(0)).toBe("0");
  });

  it("sépare les milliers selon la langue", () => {
    expect(plain(kbps(5071))).toBe("5 071 kbps");
    i18n.lang = "en";
    expect(kbps(5071)).toBe("5,071 kbps");
  });
});

describe("crédits et tonalités", () => {
  it("traduit un rôle quelle que soit sa forme, garde un rôle inconnu", () => {
    expect(creditRole("Executive Producer")).toBe("Production exécutive");
    expect(creditRole("Co-Producer")).toBe("Coproduction");
    expect(creditRole("Theremin")).toBe("Theremin");
  });

  it("écrit une tonalité dans la notation de la langue", () => {
    expect(musicalKey("FSharp", "MINOR")).toBe("Fa♯ mineur");
    expect(musicalKey("Bb", "MAJOR")).toBe("Si♭ majeur");
    expect(musicalKey("C", null)).toBe("Do");
    i18n.lang = "en";
    expect(musicalKey("FSharp", "MINOR")).toBe("F♯ minor");
  });

  it("garde une forme inconnue telle quelle", () => {
    expect(musicalKey("Cm7", "MAJOR")).toBe("Cm7 major");
    expect(musicalKey(null, "MAJOR")).toBeNull();
  });
});

describe("libellés techniques", () => {
  it("nomme les tags connus seulement", () => {
    expect(tagLabel("HIRES_LOSSLESS")).toBe("Hi-Res");
    expect(tagLabel("MQA")).toBeNull();
  });

  it("nomme les formats de sortie de mpv", () => {
    expect(outputFormat("s24")).toBe("24 bits");
    expect(outputFormat("float")).toBe("32 bits flottant");
    expect(outputFormat("offload")).toBe("décodé par la puce audio");
    expect(outputFormat("u8")).toBe("u8");
  });
});
