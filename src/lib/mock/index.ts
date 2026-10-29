// Backend factice pour prévisualiser l'UI dans un navigateur (`pnpm dev`, hors Tauri).
// Données : `fixtures.json`, généré par `cargo test dump_ui_fixtures -- --ignored`
// (gitignoré). Sans ce fichier, un jeu minimal synthétique est utilisé.
//
// Paramètres d'URL : `?theme=studio|atelier|seaglass`, `?route=album|artist|playlist|
// library|search|home`, `?nowplaying=1`, `?paused=1`, `?loggedout=1`, `?queue=1`.

import type {
  AlbumPage,
  ArtistPage,
  HomeRow,
  PlayerState,
  PlaylistPage,
  QueueView,
  SearchResults,
  Theme,
  Track,
  UiSettings,
} from "../ipc";
import type { LibraryTab, Route } from "../stores/nav.svelte";

interface Fixtures {
  search: SearchResults;
  album: AlbumPage;
  artist: ArtistPage;
  playlist: PlaylistPage;
  home: HomeRow[];
  library: { tracks: Track[]; albums: unknown[]; artists: unknown[]; playlists: unknown[] };
}

const found = import.meta.glob<Fixtures>("./fixtures.json", { eager: true, import: "default" });
const fx: Fixtures = Object.values(found)[0] ?? synthetic();
const params = new URLSearchParams(location.search);

const allTracks = new Map<number, Track>();
for (const t of [
  ...fx.album.tracks,
  ...fx.artist.topTracks,
  ...fx.playlist.tracks,
  ...fx.search.tracks,
  ...fx.library.tracks,
]) {
  allTracks.set(t.id, t);
}

let context: Track[] = fx.album.tracks;
let index = Math.min(1, context.length - 1);
const manual: Track[] = [];

const state: PlayerState = {
  track: context[index] ?? null,
  stream: { quality: "HI_RES_LOSSLESS", codec: "flac", bitDepth: 24, sampleRate: 44100 },
  output: { format: "s32", samplerate: 44100, channelCount: 2 },
  outputDriver: "wasapi",
  loading: false,
  paused: params.has("paused"),
  buffering: false,
  idle: false,
  timePos: 48,
  duration: context[index]?.duration ?? 0,
  shuffle: false,
  repeat: "off",
  hasPrevious: true,
  hasNext: true,
};

let ui: UiSettings = { theme: (params.get("theme") as Theme) ?? "seaglass" };

type Listener = (e: { payload: unknown }) => void;
const listeners = new Map<string, Set<Listener>>();

function emit(event: string, payload: unknown) {
  listeners.get(event)?.forEach((cb) => cb({ payload: structuredClone(payload) }));
}

function queueView(): QueueView {
  return { manual: [...manual], upcoming: context.slice(index + 1, index + 51) };
}

function setCurrent(track: Track | undefined) {
  if (!track) return;
  Object.assign(state, {
    track,
    timePos: 0,
    duration: track.duration,
    paused: false,
    idle: false,
    stream: track.hires
      ? { quality: "HI_RES_LOSSLESS", codec: "flac", bitDepth: 24, sampleRate: 48000 }
      : { quality: "LOSSLESS", codec: "flac", bitDepth: 16, sampleRate: 44100 },
    output: { format: track.hires ? "s32" : "s16", samplerate: track.hires ? 48000 : 44100, channelCount: 2 },
  });
  state.hasNext = manual.length > 0 || index + 1 < context.length;
  emit("player-state", state);
  emit("queue-state", queueView());
}

const handlers: Record<string, (args: Record<string, unknown>) => unknown> = {
  auth_status: () => (params.has("loggedout") ? null : { userId: 1, countryCode: "CH" }),
  login: () => ({ userId: 1, countryCode: "CH" }),
  logout: () => null,
  home: () => fx.home,
  search: ({ query }) => (String(query).trim() ? fx.search : { tracks: [], albums: [], artists: [], playlists: [] }),
  album: () => fx.album,
  artist: () => fx.artist,
  playlist: () => fx.playlist,
  library_tracks: () => fx.library.tracks,
  library_albums: () => fx.library.albums,
  library_artists: () => fx.library.artists,
  library_playlists: () => fx.library.playlists,
  player_state: () => state,
  queue_state: () => queueView(),
  ui_settings: () => ui,
  set_ui_settings: ({ settings }) => {
    ui = settings as UiSettings;
  },
  play_tracks: ({ trackIds, startIndex, shuffle }) => {
    context = (trackIds as number[]).map((id) => allTracks.get(id)).filter((t): t is Track => !!t);
    index = shuffle ? Math.floor(Math.random() * context.length) : Number(startIndex);
    state.shuffle = state.shuffle || Boolean(shuffle);
    setCurrent(context[index]);
  },
  play_next: ({ trackId }) => {
    const t = allTracks.get(Number(trackId));
    if (t) manual.unshift(t);
    emit("queue-state", queueView());
  },
  add_to_queue: ({ trackId }) => {
    const t = allTracks.get(Number(trackId));
    if (t) manual.push(t);
    emit("queue-state", queueView());
  },
  remove_from_queue: ({ index: i }) => {
    manual.splice(Number(i), 1);
    emit("queue-state", queueView());
  },
  next: () => {
    if (manual.length) return setCurrent(manual.shift());
    if (index + 1 < context.length) setCurrent(context[++index]);
  },
  previous: () => {
    if (state.timePos > 3 || index === 0) {
      state.timePos = 0;
      emit("player-state", state);
    } else setCurrent(context[--index]);
  },
  toggle_pause: () => {
    state.paused = !state.paused;
    emit("player-state", state);
  },
  seek: ({ seconds }) => {
    state.timePos = Number(seconds);
    emit("player-state", state);
  },
  set_shuffle: ({ enabled }) => {
    state.shuffle = Boolean(enabled);
    emit("player-state", state);
  },
  set_repeat: ({ mode }) => {
    state.repeat = mode as PlayerState["repeat"];
    emit("player-state", state);
  },
};

export async function invoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const handler = handlers[cmd];
  if (!handler) throw new Error(`commande factice inconnue : ${cmd}`);
  // Petite latence pour voir les états de chargement.
  await new Promise((r) => setTimeout(r, 120));
  return structuredClone(handler(args)) as T;
}

export async function listen<T>(event: string, cb: (e: { payload: T }) => void) {
  const set = listeners.get(event) ?? new Set();
  set.add(cb as Listener);
  listeners.set(event, set);
  return () => set.delete(cb as Listener);
}

/** Paramètres d'URL de navigation, appliqués une fois l'app montée. */
export function initialView(): { route: Route | null; nowPlaying: boolean; queue: boolean } {
  const routes: Record<string, Route> = {
    album: { name: "album", id: fx.album.album.id },
    artist: { name: "artist", id: fx.artist.artist.id },
    playlist: { name: "playlist", uuid: fx.playlist.playlist.uuid },
    library: { name: "library", tab: (params.get("tab") as LibraryTab) ?? "playlists" },
    search: { name: "search" },
  };
  return {
    route: routes[params.get("route") ?? ""] ?? null,
    nowPlaying: params.has("nowplaying"),
    queue: params.has("queue"),
  };
}

function synthetic(): Fixtures {
  const track = (id: number, title: string): Track => ({
    id,
    title,
    version: null,
    duration: 200 + id,
    artists: [{ id: 1, name: "Artiste de démonstration" }],
    album: { id: 1, title: "Album de démonstration", coverUrl: null, color: "#4f95b0" },
    explicit: false,
    hires: id % 2 === 0,
  });
  const tracks = Array.from({ length: 12 }, (_, i) => track(i + 1, `Titre ${i + 1}`));
  const album = {
    id: 1,
    title: "Album de démonstration",
    artists: [{ id: 1, name: "Artiste de démonstration" }],
    coverUrl: null,
    color: "#4f95b0",
    releaseDate: "2024-01-01",
    numberOfTracks: 12,
    albumType: "ALBUM",
    explicit: false,
    hires: true,
  };
  return {
    search: { tracks, albums: [album], artists: [], playlists: [] },
    album: { album, coverLargeUrl: null, duration: 2600, copyright: null, tracks },
    artist: {
      artist: { id: 1, name: "Artiste de démonstration", pictureUrl: null },
      topTracks: tracks.slice(0, 5),
      albums: [album],
      singles: [],
    },
    playlist: {
      playlist: {
        uuid: "00000000-0000-0000-0000-000000000000",
        title: "Playlist de démonstration",
        description: null,
        imageUrl: null,
        numberOfTracks: 12,
        duration: 2600,
        editorial: true,
        mine: false,
      },
      tracks,
    },
    home: [{ title: "Pour vous", items: [{ kind: "album", ...album }] }],
    library: { tracks, albums: [album], artists: [], playlists: [] },
  };
}
