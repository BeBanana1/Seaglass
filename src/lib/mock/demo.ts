// Invented catalog of the mock backend (#28): used by the website demo ("Try Seaglass") and by
// `pnpm dev` when `fixtures.json` is missing (or with `?demo=1`). Every artist, album, track and
// cover here is made up: nothing comes from TIDAL or from an account, so it can be public.
// Covers are the SVG files of `./demo/`, emitted as assets only when the mock is part of the
// build (never in the app's production build).

import type {
  AlbumPage,
  AlbumSummary,
  ArtistPage,
  ArtistSummary,
  FeedItem,
  HomeRow,
  MixPage,
  MixSummary,
  PlaylistPage,
  PlaylistSummary,
  Track,
  TrackEntry,
} from "../ipc";
import type { Fixtures } from "./index";

const files = import.meta.glob<string>("./demo/*.svg", {
  eager: true,
  query: "?url",
  import: "default",
});
const image = (name: string): string | null => files[`./demo/${name}.svg`] ?? null;

const ARTISTS: ArtistSummary[] = [
  "Lumen Tide",
  "The Night Ferry",
  "Paper Moons",
  "Low Static",
  "Coral Atlas",
  "Velvet Orbit",
].map((name, i) => ({ id: i + 1, name, pictureUrl: image(`artist-${i + 1}`) }));

interface AlbumDef {
  title: string;
  artist: number;
  cover: string;
  color: string;
  date: string;
  type: "ALBUM" | "EP" | "SINGLE";
  hires: boolean;
  tracks: (string | [string, string])[];
  explicit?: number[];
}

const ALBUMS: AlbumDef[] = [
  {
    title: "Tidewater",
    artist: 1,
    cover: "tidewater",
    color: "#5fb3b3",
    date: "2026-03-13",
    type: "ALBUM",
    hires: true,
    tracks: [
      "First Light",
      "Sea Glass",
      "Undertow",
      "Harbour Walls",
      "Low Tide Choir",
      "Lanterns",
      "Driftwood",
      "Saltwater Hymn",
      "The Long Shore",
      "Homeward",
    ],
  },
  {
    title: "Neon Harbor",
    artist: 2,
    cover: "neon-harbor",
    color: "#c2367a",
    date: "2025-10-24",
    type: "ALBUM",
    hires: false,
    tracks: [
      "Night Ferry",
      "Midnight Pier",
      "Neon Rain",
      "Signal Fires",
      "Crossing",
      "Port of Call",
      "Last Boat Home",
    ],
    explicit: [2],
  },
  {
    title: "Small Hours",
    artist: 3,
    cover: "paper-moons",
    color: "#d9c7a7",
    date: "2024-11-08",
    type: "ALBUM",
    hires: true,
    tracks: [
      "Small Hours",
      "Paper Boats",
      "Moonlit Kitchen",
      "Quiet Machines",
      "Snowfall Radio",
      "Letters",
    ],
  },
  {
    title: "Signal / Noise",
    artist: 4,
    cover: "low-static",
    color: "#9aa0a6",
    date: "2026-06-05",
    type: "EP",
    hires: false,
    tracks: ["Signal", "Noise", "Carrier Wave", "Static Bloom"],
  },
  {
    title: "Warm Currents",
    artist: 5,
    cover: "coral-atlas",
    color: "#ff7a59",
    date: "2025-07-18",
    type: "ALBUM",
    hires: true,
    tracks: [
      "Warm Currents",
      "Reef",
      "Gulf Stream",
      "Shallows",
      "Atlas",
      "Tidepool",
      "Sunset Line",
      "Northbound",
    ],
  },
  {
    title: "Orbit Songs",
    artist: 6,
    cover: "velvet-orbit",
    color: "#7b2cbf",
    date: "2026-09-26",
    type: "ALBUM",
    hires: true,
    tracks: [
      "Perihelion",
      "Slow Orbit",
      "Velvet Sky",
      "Satellite Heart",
      "Gravity Well",
      "Afterglow",
      "Escape Velocity",
    ],
  },
  {
    title: "Salt and Pine",
    artist: 3,
    cover: "salt-and-pine",
    color: "#7fa37a",
    date: "2022-04-22",
    type: "ALBUM",
    hires: false,
    tracks: ["Pine Needles", "Salt Road", "Cabin Lights", "Woodsmoke", "Evergreen", "River Mouth"],
  },
  {
    title: "Glass Garden",
    artist: 1,
    cover: "glass-garden",
    color: "#6fffe9",
    date: "2026-11-06",
    type: "SINGLE",
    hires: true,
    tracks: ["Glass Garden", ["Glass Garden", "Night Version"]],
  },
];

/** Durations between 2:50 and 5:30, fixed from one load to the next. */
const duration = (id: number) => 170 + ((id * 7919) % 160);

const albumPages: AlbumPage[] = ALBUMS.map((def, a) => {
  const id = 1001 + a;
  const artist = ARTISTS[def.artist - 1];
  const cover = image(def.cover);
  const album: AlbumSummary = {
    id,
    title: def.title,
    artists: [{ id: artist.id, name: artist.name }],
    coverUrl: cover,
    coverLargeUrl: cover,
    color: def.color,
    releaseDate: def.date,
    numberOfTracks: def.tracks.length,
    albumType: def.type,
    explicit: (def.explicit ?? []).length > 0,
    ai: false,
    hires: def.hires,
  };
  const tracks: Track[] = def.tracks.map((entry, n) => {
    const [title, version] = typeof entry === "string" ? [entry, null] : entry;
    const trackId = id * 100 + n + 1;
    return {
      id: trackId,
      title,
      version,
      duration: duration(trackId),
      artists: [{ id: artist.id, name: artist.name }],
      album: {
        id,
        title: def.title,
        coverUrl: cover,
        coverSmallUrl: cover,
        coverLargeUrl: cover,
        color: def.color,
      },
      explicit: (def.explicit ?? []).includes(n + 1),
      ai: false,
      hires: def.hires,
      isrc: null,
    };
  });
  return {
    album,
    coverLargeUrl: cover,
    duration: tracks.reduce((sum, t) => sum + t.duration, 0),
    copyright: `℗ ${def.date.slice(0, 4)} ${artist.name} (fictional, Seaglass demo)`,
    tracks,
  };
});

const albums = albumPages.map((p) => p.album);
const allTracks = albumPages.flatMap((p) => p.tracks);
const track = (album: number, n: number) => albumPages[album].tracks[n - 1];

const artistPages: ArtistPage[] = ARTISTS.map((artist) => {
  const own = albumPages.filter((p) => p.album.artists[0].id === artist.id);
  const tracks = own.flatMap((p) => p.tracks);
  return {
    artist,
    topTracks: [...tracks].sort((a, b) => duration(b.id * 3) - duration(a.id * 3)).slice(0, 5),
    albums: own.map((p) => p.album).filter((a) => a.albumType !== "SINGLE"),
    singles: own.map((p) => p.album).filter((a) => a.albumType === "SINGLE"),
  };
});

/** Dates added: in no particular order in a playlist, newest first for favorites. */
const day = (d: number) => Date.UTC(2026, 0, 1) + d * 86_400_000;
const entries = (tracks: Track[], added: (i: number) => number): TrackEntry[] =>
  tracks.map((t, i) => ({ ...t, addedAt: added(i) }));

function playlist(
  n: number,
  title: string,
  description: string,
  tracks: Track[],
  mine = false,
): PlaylistPage {
  const summary: PlaylistSummary = {
    uuid: `de300000-0000-4000-8000-00000000000${n}`,
    title,
    description,
    imageUrl: image(`playlist-${((n - 1) % 3) + 1}`),
    numberOfTracks: tracks.length,
    duration: tracks.reduce((sum, t) => sum + t.duration, 0),
    editorial: !mine,
    mine,
  };
  return {
    playlist: summary,
    tracks: entries(tracks, (i) => day(10 + ((i * 7) % tracks.length))),
    revision: null,
  };
}

const playlistPages: PlaylistPage[] = [
  playlist(1, "Sea Glass Sessions", "Calm, bright and lossless: the demo's house playlist.", [
    track(0, 2),
    track(4, 1),
    track(2, 1),
    track(5, 3),
    track(0, 6),
    track(6, 3),
    track(1, 3),
    track(4, 6),
    track(0, 9),
    track(2, 4),
    track(5, 6),
    track(3, 4),
    track(7, 1),
    track(6, 6),
  ]),
  playlist(2, "Late Night Drive", "Neon, rain and empty roads.", [
    track(1, 1),
    track(1, 2),
    track(5, 5),
    track(3, 1),
    track(1, 4),
    track(5, 7),
    track(3, 3),
    track(1, 7),
    track(7, 2),
  ]),
  playlist(3, "Focus Flow", "Long, quiet tracks to work to.", [
    track(2, 4),
    track(0, 5),
    track(6, 4),
    track(4, 4),
    track(2, 6),
    track(0, 7),
    track(6, 1),
    track(4, 8),
  ]),
  playlist(
    4,
    "Weekend Mornings",
    "",
    [track(2, 2), track(6, 2), track(4, 7), track(0, 1), track(5, 2), track(0, 10)],
    true,
  ),
];
const playlists = playlistPages.map((p) => p.playlist);

function mix(n: number, title: string, subtitle: string, type: string, tracks: Track[]): MixPage {
  const summary: MixSummary = {
    id: `de30000000000000000000000000000${n}`,
    title,
    subtitle,
    imageUrl: image(`mix-${n}`),
    mixType: type,
  };
  return { mix: summary, imageLargeUrl: summary.imageUrl, tracks };
}

const mixPages: MixPage[] = [
  mix(1, "My Mix 1", "Lumen Tide, Coral Atlas and more", "DAILY_MIX", [
    track(0, 3),
    track(4, 2),
    track(0, 8),
    track(4, 5),
    track(7, 1),
    track(4, 3),
    track(0, 4),
    track(6, 5),
  ]),
  mix(2, "Discovery Mix", "New music picked for you", "DISCOVERY_MIX", [
    track(5, 1),
    track(3, 2),
    track(1, 5),
    track(5, 4),
    track(2, 5),
    track(1, 6),
    track(3, 4),
  ]),
];
const mixes = mixPages.map((p) => p.mix);

const media = {
  album: (a: AlbumSummary) => ({ kind: "album" as const, ...a }),
  artist: (a: ArtistSummary) => ({ kind: "artist" as const, ...a }),
  playlist: (p: PlaylistSummary) => ({ kind: "playlist" as const, ...p }),
  mix: (m: MixSummary) => ({ kind: "mix" as const, ...m }),
};

const home: HomeRow[] = [
  {
    key: "SHORTCUT_LIST",
    title: "Shortcuts",
    items: [media.mix(mixes[0]), media.playlist(playlists[0]), media.album(albums[0])],
  },
  { key: "ALBUM_RECOMMENDATIONS", title: "Albums you might like", items: albums.map(media.album) },
  { key: "DAILY_MIXES", title: "Mixes made for you", items: mixes.map(media.mix) },
  {
    key: "POPULAR_PLAYLISTS",
    title: "Popular playlists",
    items: playlists.filter((p) => p.editorial).map(media.playlist),
  },
  {
    key: "YOUR_FAVORITE_ARTISTS",
    title: "Your favorite artists",
    items: ARTISTS.map(media.artist),
  },
  {
    key: "NEW_ALBUM_SUGGESTIONS",
    title: "New albums for you",
    items: [...albums]
      .sort((a, b) => (b.releaseDate ?? "").localeCompare(a.releaseDate ?? ""))
      .map(media.album),
  },
];

const feed: FeedItem[] = [
  { occurredAt: Date.now() - 3_600_000, seen: false, item: media.album(albums[7]) },
  { occurredAt: day(268), seen: true, item: media.album(albums[5]) },
  { occurredAt: day(155), seen: true, item: media.album(albums[3]) },
  { occurredAt: day(71), seen: true, item: media.album(albums[0]) },
];

/** The demo catalog, in the shape of the fixtures. */
export function demoFixtures(): Fixtures {
  const liked = [
    track(0, 2),
    track(4, 1),
    track(5, 3),
    track(2, 1),
    track(1, 3),
    track(6, 3),
    track(0, 9),
    track(3, 4),
    track(7, 1),
    track(4, 6),
    track(5, 6),
    track(2, 4),
  ];
  return {
    search: {
      tracks: allTracks.slice(0, 12),
      albums,
      artists: ARTISTS,
      playlists: playlists.filter((p) => p.editorial),
    },
    album: albumPages[0],
    artist: artistPages[0],
    playlist: playlistPages[0],
    mix: mixPages[0],
    radio: {
      seed: "track",
      id: track(0, 2).id,
      title: track(0, 2).title,
      imageUrl: track(0, 2).album.coverUrl,
      tracks: [track(0, 2), track(4, 4), track(2, 3), track(5, 2), track(6, 5), track(1, 4)],
    },
    home,
    feed,
    library: {
      tracks: entries(liked, (i) => day(320 - i * 9)),
      albums: [albums[0], albums[4], albums[5], albums[2]],
      artists: [ARTISTS[0], ARTISTS[4], ARTISTS[5]],
      playlists: [playlists[3], playlists[0], playlists[1]],
      mixes,
    },
    pages: { albums: albumPages, artists: artistPages, playlists: playlistPages, mixes: mixPages },
  };
}

/** Search over the demo catalog: words of the query found in a title or a name. */
export function searchDemo(query: string) {
  const words = query.toLowerCase().split(" ").filter(Boolean);
  const hit = (...texts: string[]) => {
    const text = texts.join(" ").toLowerCase();
    return words.every((w) => text.includes(w));
  };
  return {
    tracks: allTracks
      .filter((t) => hit(t.title, t.version ?? "", t.album.title, ...t.artists.map((a) => a.name)))
      .slice(0, 20),
    albums: albums.filter((a) => hit(a.title, ...a.artists.map((x) => x.name))),
    artists: ARTISTS.filter((a) => hit(a.name)),
    playlists: playlists.filter((p) => p.editorial && hit(p.title, p.description ?? "")),
  };
}
