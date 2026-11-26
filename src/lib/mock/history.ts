// Listening history in the mock backend: play history, stats, recap, smart lists and
// ListenBrainz.

import { fx, params, type Handlers } from "./core";
import type { AlbumSummary, ListenBrainzStatus } from "../ipc";

/** Mock listening history (#202): a few albums from the fixtures. */
export let playedAlbums: AlbumSummary[] = fx.search.albums.slice(0, 5);

// `?listenbrainz=1`: account connected; `=refused`: token refused since (#431).
const lbParam = params.get("listenbrainz");
let listenbrainz: ListenBrainzStatus = {
  user: lbParam ? "seaglass-fan" : null,
  refused: lbParam === "refused",
  pending: lbParam ? 3 : 0,
};

export const historyHandlers: Handlers = {
  play_history: () => ({ recent: playedAlbums, top: playedAlbums.slice(0, 2) }),
  clear_play_history: () => void (playedAlbums = []),
  play_stats: ({ period }) => {
    if (playedAlbums.length === 0)
      return { plays: 0, seconds: 0, unknown: 0, tracks: [], albums: [], artists: [] };
    const scale =
      ({ week: 1, month: 4, year: 30, all: 40 } as Record<string, number>)[String(period)] ?? 1;
    const tracks = fx.search.tracks
      .slice(0, 12)
      .map((item, i) => ({ item, plays: (12 - i) * scale }));
    const artists = fx.search.artists
      .slice(0, 6)
      .map((a, i) => ({ item: a, plays: (9 - i) * scale }));
    return {
      plays: tracks.reduce((n, t) => n + t.plays, 0),
      seconds: tracks.reduce((n, t) => n + t.plays * t.item.duration, 0),
      unknown: period === "all" ? 37 : 0,
      tracks,
      albums: playedAlbums.map((item, i) => ({ item, plays: (8 - i) * scale })),
      artists,
    };
  },
  // Recap (#417): the last two months and their year, with figures from the fixtures.
  recap_periods: () => {
    if (playedAlbums.length === 0) return [];
    const now = new Date();
    const prev = new Date(now.getFullYear(), now.getMonth() - 1, 1);
    const months = [now, prev].map((d) => ({ year: d.getFullYear(), month: d.getMonth() + 1 }));
    const out: { year: number; month: number | null }[] = [];
    for (const m of months) {
      if (out.at(-1)?.year !== m.year) out.push({ year: m.year, month: null });
      out.push(m);
    }
    return out;
  },
  recap: ({ month }) => {
    const scale = month == null ? 12 : 1;
    const tracks = fx.search.tracks
      .slice(0, 10)
      .map((item, i) => ({ item, plays: (14 - i) * scale }));
    const artists = fx.search.artists
      .slice(0, 6)
      .map((item, i) => ({ item, plays: (11 - i) * scale }));
    const hours = Array.from(
      { length: 24 },
      (_, h) => Math.round((h >= 8 ? 3 : 0) + (h >= 18 ? 6 : 0) + (h === 22 ? 4 : 0)) * scale,
    );
    const weekdays = [5, 4, 6, 4, 9, 12, 8].map((n) => n * scale);
    const days =
      month == null ? 12 : new Date(new Date().getFullYear(), Number(month), 0).getDate();
    const timeline = Array.from({ length: days }, (_, i) =>
      month == null ? 40 + ((i * 37) % 60) : (i * 7) % 5 === 0 ? 0 : 3 + ((i * 5) % 9),
    );
    return {
      plays: tracks.reduce((n, t) => n + t.plays, 0),
      seconds: tracks.reduce((n, t) => n + t.plays * t.item.duration, 0),
      unknown: 0,
      days: month == null ? 214 : 23,
      tracks,
      albums: playedAlbums.map((item, i) => ({ item, plays: (9 - i) * scale })),
      artists,
      newArtists: artists.slice(3).map((a) => ({ ...a, plays: a.plays - 3 })),
      newArtistCount: 7 * scale,
      newTrackCount: 31 * scale,
      weekdays,
      hours,
      topWeekday: 5,
      topHour: 22,
      timeline,
      // Hours spread over each day in proportion to that day's plays.
      grid: weekdays.map((w) => hours.map((h) => Math.round((h * w) / 60))),
      streak: month == null ? 19 : 9,
      topDay: { track: tracks[1].item, plays: 7, date: `${new Date().getFullYear()}-03-14` },
      firstTrack: tracks[4]?.item ?? null,
      audio: {
        known: 96 * scale,
        hires: 41 * scale,
        lossless: 49 * scale,
        lossy: 6 * scale,
        bitPerfect: 58 * scale,
        bitPerfectSeconds: 58 * 230 * scale,
        formats: [
          { codec: "flac", bitDepth: 16, sampleRate: 44100, plays: 49 * scale },
          { codec: "flac", bitDepth: 24, sampleRate: 96000, plays: 23 * scale },
          { codec: "flac", bitDepth: 24, sampleRate: 48000, plays: 12 * scale },
          { codec: "flac", bitDepth: 24, sampleRate: 192000, plays: 6 * scale },
          { codec: "mp4a.40.2", bitDepth: null, sampleRate: 44100, plays: 6 * scale },
        ],
      },
    };
  },
  // Smart lists (#383): empty while the history is, like the real backend.
  smart_list: ({ list }) => {
    if (playedAlbums.length === 0) return [];
    const offset =
      ({ topMonth: 0, onRepeat: 2, forgotten: 4, neverPlayed: 6 } as Record<string, number>)[
        String(list)
      ] ?? 0;
    return fx.search.tracks.slice(offset, offset + 8);
  },
  // A token containing "bad" is refused, as by `validate-token`.
  listenbrainz_status: () => listenbrainz,
  listenbrainz_connect: ({ token }) => {
    const value = String(token).trim();
    if (!value || /\s/.test(value)) throw "jeton ListenBrainz invalide : collez-le tel quel";
    if (value.includes("bad"))
      throw "jeton refusé par ListenBrainz : copiez-le depuis vos réglages ListenBrainz";
    listenbrainz = { user: "seaglass-fan", refused: false, pending: listenbrainz.pending };
    return listenbrainz;
  },
  listenbrainz_disconnect: () => (listenbrainz = { user: null, refused: false, pending: 0 }),
};
