import { describe, expect, it } from "vitest";
import type { Recap, Track } from "./ipc";
import { recapCardText, TOP, type RecapLabels } from "./recapCard";

const labels: RecapLabels = {
  kicker: "Recap",
  time: (h, m) => (h > 0 ? `${h} h ${m} min` : `${m} min`),
  timeLabel: "Listening time",
  playsLabel: "Plays",
  daysLabel: "Listening days",
  newArtistsLabel: "New artists",
  topLabel: "Most played tracks",
  plays: (n) => `${n} plays`,
  num: (n) => String(n),
};

function track(id: number): Track {
  return {
    id,
    title: `Song ${id}`,
    version: id === 1 ? "Live" : null,
    duration: 200,
    artists: [
      { id: 1, name: "A" },
      { id: 2, name: "B" },
    ],
    album: { id, title: "Album", coverUrl: `https://img/${id}.jpg`, color: null },
    explicit: false,
    ai: false,
    hires: false,
  } as Track;
}

const recap = {
  plays: 412,
  seconds: 3 * 3600 + 25 * 60 + 20,
  days: 18,
  newArtistCount: 7,
  tracks: Array.from({ length: 8 }, (_, i) => ({ item: track(i + 1), plays: 20 - i })),
} as unknown as Recap;

describe("recapCardText", () => {
  const text = recapCardText(recap, "November 2026", labels);

  it("has the four numbers, time rounded to the minute", () => {
    expect(text.stats).toEqual([
      { value: "3 h 25 min", label: "Listening time" },
      { value: "412", label: "Plays" },
      { value: "18", label: "Listening days" },
      { value: "7", label: "New artists" },
    ]);
  });

  it("keeps only the top tracks, with full title, artists, plays and cover", () => {
    expect(text.top).toHaveLength(TOP);
    expect(text.top[0]).toEqual({
      title: "Song 1 (Live)",
      artists: "A, B",
      plays: "20 plays",
      cover: "https://img/1.jpg",
    });
  });

  it("carries the period and a footer", () => {
    expect(text.period).toBe("November 2026");
    expect(text.footer).toContain("Seaglass");
  });

  it("has fewer rows when fewer tracks were played", () => {
    const short = { ...recap, tracks: recap.tracks.slice(0, 2) } as Recap;
    expect(recapCardText(short, "2026", labels).top).toHaveLength(2);
  });
});
