// Backend factice pour prévisualiser l'UI dans un navigateur (`pnpm dev`, hors Tauri).
// Données : `fixtures.json`, généré par `cargo test dump_ui_fixtures -- --ignored`
// (gitignoré). Sans ce fichier, un jeu minimal synthétique est utilisé.
//
// Paramètres d'URL : `?theme=studio|atelier|seaglass`, `?route=album|artist|playlist|
// library|search|home`, `?nowplaying=1`, `?paused=1`, `?loggedout=1`, `?queue=1`, `?update=1`,
// `?volume=60|muted`.

import type {
  AlbumPage,
  ArtistPage,
  HomeLayout,
  HomeRow,
  HomeSection,
  PlayerState,
  PlaylistPage,
  QueueView,
  SearchResults,
  Theme,
  Track,
  TrackEntry,
  UiSettings,
} from "../ipc";
import type { LibraryTab, Route } from "../stores/nav.svelte";

interface Fixtures {
  search: SearchResults;
  album: AlbumPage;
  artist: ArtistPage;
  playlist: PlaylistPage;
  home: HomeRow[];
  library: { tracks: TrackEntry[]; albums: unknown[]; artists: unknown[]; playlists: unknown[] };
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
/** `?volume=60` ou `?volume=muted` : chaîne audio hors bit-perfect. */
const volumeParam = params.get("volume");

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
  volume: {
    level: volumeParam && volumeParam !== "muted" ? Math.min(100, Math.max(0, Number(volumeParam) || 0)) : 100,
    muted: volumeParam === "muted",
  },
};

let ui: UiSettings = {
  theme: (params.get("theme") as Theme) ?? "seaglass",
  home: { order: [], hidden: [] },
};

/** Version simplifiée de `HomeLayout::arrange` : sections connues dans l'ordre enregistré,
 *  les autres à leur place dans le flux. Le flux factice ne change pas. */
function arrangeHome(rows: HomeRow[], layout: HomeLayout): HomeSection[] {
  const keyed = rows.map((r) => ({ ...r, key: r.key ?? `title:${r.title.toLowerCase()}` }));
  const rank = (k: string) => {
    const i = layout.order.indexOf(k);
    return i < 0 ? keyed.findIndex((r) => r.key === k) - 0.5 : i;
  };
  return keyed
    .map((r, i) => ({ r, i }))
    .sort((a, b) => rank(a.r.key) - rank(b.r.key) || a.i - b.i)
    .map(({ r }) => ({ ...r, hidden: layout.hidden.includes(r.key) }));
}

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
  home: () => arrangeHome(fx.home, ui.home),
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
  // `?update=1` : simule une mise à jour prête.
  update_status: () =>
    params.has("update")
      ? {
          version: "0.5.0",
          currentVersion: "0.4.4",
          notes: ["- Installeur Windows", "- Mises à jour automatiques"].join("\n"),
        }
      : null,
  install_update: () => null,
  set_ui_settings: ({ settings }) => {
    ui = settings as UiSettings;
  },
  set_home_layout: ({ sections }) => {
    const list = sections as { key: string; hidden: boolean }[];
    ui.home = { order: list.map((s) => s.key), hidden: list.filter((s) => s.hidden).map((s) => s.key) };
    return ui.home;
  },
  reset_home_layout: () => (ui.home = { order: [], hidden: [] }),
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
  enqueue_tracks: ({ trackIds, next }) => {
    const ts = (trackIds as number[]).map((id) => allTracks.get(id)).filter((t): t is Track => !!t);
    if (next) manual.unshift(...ts);
    else manual.push(...ts);
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
  // Mêmes règles que `player::volume::Volume` côté Rust.
  set_volume: ({ volume }) => {
    const level = Number(volume);
    if (!Number.isInteger(level) || level < 0 || level > 100) throw new Error("volume invalide");
    state.volume.level = level;
    if (level > 0) state.volume.muted = false;
    emit("player-state", state);
  },
  set_muted: ({ muted }) => {
    state.volume.muted = Boolean(muted);
    if (!state.volume.muted && state.volume.level === 0) state.volume.level = 50;
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
  // Dates d'ajout dans le désordre (playlist) ou décroissantes (favoris), comme l'API.
  const day = (d: number) => Date.UTC(2024, 0, d);
  const playlistTracks = tracks.map((t, i) => ({ ...t, addedAt: day(1 + ((i * 5) % 12)) }));
  const likedTracks = tracks.map((t, i) => ({ ...t, addedAt: day(12 - i) }));
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
      tracks: playlistTracks,
    },
    home: [{ key: "FOR_YOU", title: "Pour vous", items: [{ kind: "album", ...album }] }],
    library: { tracks: likedTracks, albums: [album], artists: [], playlists: [] },
  };
}
