import { describe, expect, it } from "vitest";
import type { StreamFormat, Track } from "./ipc";
import { cardText, fitText, formatLine } from "./shareCard";

/** One unit per character: easy widths to reason about. */
const measure = (s: string) => Array.from(s).length;

const hires: StreamFormat = {
  quality: "HI_RES_LOSSLESS",
  codec: "flac",
  bitDepth: 24,
  sampleRate: 96000,
  bitrate: null,
  hiresEdition: null,
};

const track: Track = {
  id: 1,
  title: "Song",
  version: "Live",
  duration: 200,
  artists: [
    { id: 1, name: "A" },
    { id: 2, name: "B" },
  ],
  album: { id: 3, title: "Album", coverUrl: null, color: null },
  explicit: false,
  ai: false,
  hires: true,
} as Track;

describe("fitText", () => {
  it("keeps a text that fits", () => {
    expect(fitText("short", 10, measure)).toBe("short");
  });

  it("cuts with an ellipsis, within the width", () => {
    const out = fitText("a long title here", 8, measure);
    expect(out).toBe("a long…");
    expect(measure(out)).toBeLessThanOrEqual(8);
  });

  it("does not split surrogate pairs", () => {
    expect(fitText("🎵🎵🎵🎵", 3, measure)).toBe("🎵🎵…");
  });
});

describe("formatLine", () => {
  it("adds the verdict only when bit-perfect", () => {
    expect(formatLine(hires, { tone: "ok", label: "Bit-perfect", detail: "" })).toBe(
      "FLAC 24/96 · Bit-perfect",
    );
    expect(formatLine(hires, { tone: "warn", label: "Resampled", detail: "" })).toBe("FLAC 24/96");
  });

  it("has nothing without a stream", () => {
    expect(formatLine(null, { tone: "idle", label: "", detail: "" })).toBeNull();
  });
});

describe("cardText", () => {
  it("takes the full title, every artist and the album", () => {
    const text = cardText(track, hires, { tone: "ok", label: "Bit-perfect", detail: "" });
    expect(text).toMatchObject({
      title: "Song (Live)",
      artists: "A, B",
      album: "Album",
      format: "FLAC 24/96 · Bit-perfect",
    });
    expect(text.footer).toContain("Seaglass");
  });
});
