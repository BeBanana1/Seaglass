import { demoFixtures } from "./demo";
import type {
  AlbumPage,
  ArtistPage,
  FeedItem,
  HomeRow,
  MixPage,
  MixSummary,
  PlaylistPage,
  PlaylistSummary,
  RadioPage,
  SearchResults,
  Track,
  TrackEntry,
} from "../ipc";

export interface Fixtures {
  search: SearchResults;
  album: AlbumPage;
  artist: ArtistPage;
  playlist: PlaylistPage;
  /** Absents des fixtures générées avant les mixes : remplacés par un jeu synthétique. */
  mix?: MixPage | null;
  radio?: RadioPage;
  home: HomeRow[];
  /** Absent des fixtures générées avant le fil d'actualité. */
  feed?: FeedItem[];
  library: {
    tracks: TrackEntry[];
    albums: unknown[];
    artists: unknown[];
    playlists: PlaylistSummary[];
    mixes?: MixSummary[];
  };
  /** Pages per identifier (invented catalog of `demo.ts`, #28): without them, every album,
   *  artist, playlist or mix opens the single page above. */
  pages?: {
    albums: AlbumPage[];
    artists: ArtistPage[];
    playlists: PlaylistPage[];
    mixes: MixPage[];
  };
}

export type Handlers = Record<string, (args: Record<string, unknown>) => unknown>;

export const params = new URLSearchParams(location.search);
// In the demo build, `fixtures.json` loads as `null` (vite.config.ts): the invented catalog
// is always used there. `?demo=1` forces it in `pnpm dev` too.
const found = import.meta.glob<Fixtures | null>("./fixtures.json", {
  eager: true,
  import: "default",
});
export const fx: Fixtures = (params.has("demo") ? null : Object.values(found)[0]) ?? demoFixtures();

const demoMix: MixSummary = {
  id: "0000000000000000000000000000ff",
  title: "My Mix 1",
  subtitle: "Artiste de démonstration et d'autres",
  imageUrl: null,
  mixType: "DAILY_MIX",
};
export const mixPage: MixPage = fx.mix ?? {
  mix: demoMix,
  imageLargeUrl: null,
  tracks: fx.album.tracks.slice(0, 10),
};
export const radioPage: RadioPage = fx.radio ?? {
  seed: "track",
  id: fx.album.tracks[0]?.id ?? 1,
  title: fx.album.tracks[0]?.title ?? "Titre",
  imageUrl: fx.album.tracks[0]?.album.coverUrl ?? null,
  tracks: [...fx.album.tracks].reverse(),
};
export const libraryMixes: MixSummary[] = fx.library.mixes ?? [mixPage.mix];

export const allTracks = new Map<number, Track>();
for (const t of [
  ...fx.album.tracks,
  ...fx.artist.topTracks,
  ...fx.playlist.tracks,
  ...fx.search.tracks,
  ...fx.library.tracks,
  ...mixPage.tracks,
  ...radioPage.tracks,
  ...(fx.pages?.albums.flatMap((p) => p.tracks) ?? []),
]) {
  allTracks.set(t.id, t);
}

/** `?long=<n>` : n titres synthétiques (les titres connus répétés, identifiants uniques). */
export const longCount = Math.min(10_000, Math.max(0, Number(params.get("long")) || 0));
export const longTracks: TrackEntry[] = (() => {
  const base = [...allTracks.values()];
  if (!longCount || base.length === 0) return [];
  const now = Date.now();
  return Array.from({ length: longCount }, (_, i) => {
    const t = base[i % base.length];
    // Le premier passage garde les vrais identifiants : le titre en cours y figure.
    const entry = {
      ...t,
      id: i < base.length ? t.id : t.id * 100_000 + i,
      addedAt: now - i * 3_600_000,
    };
    allTracks.set(entry.id, entry);
    return entry;
  });
})();
export const longPlaylist: PlaylistPage | null = longCount
  ? {
      playlist: { ...fx.playlist.playlist, numberOfTracks: longCount },
      tracks: longTracks,
      revision: null,
    }
  : null;

export type Listener = (e: { payload: unknown }) => void;
export const listeners = new Map<string, Set<Listener>>();

export function emit(event: string, payload: unknown) {
  listeners.get(event)?.forEach((cb) => cb({ payload: structuredClone(payload) }));
}
