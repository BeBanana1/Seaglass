// Backend factice pour prévisualiser l'UI dans un navigateur (`pnpm dev`, hors Tauri).
// Données : `fixtures.json`, généré par `cargo test dump_ui_fixtures -- --ignored`
// (gitignoré). Sans ce fichier, un jeu minimal synthétique est utilisé.
//
// Paramètres d'URL : `?theme=studio|atelier|seaglass`, `?route=album|artist|playlist|
// mix|radio|library|search|home|feed|settings`, `?tab=` (onglet de la bibliothèque ou section des
// réglages : appearance|playback|app), `?nowplaying=1`, `?paused=1`, `?loggedout=1`,
// `?queue=1`, `?update=1`, `?autostart=1`, `?volume=60|muted`, `?accent=ff5c8a`,
// `?lyrics=1|plain|none` (ouvre le panneau Paroles ; texte simple ou aucune parole),
// `?mine=1` (la playlist de démonstration devient la vôtre : retrait de titres possible),
// `?trackinfo=1` (ouvre les infos du titre en cours), `?sleep=30|track` (minuteur de mise en
// veille actif).

import type {
  AlbumPage,
  ArtistPage,
  FeedItem,
  HomeLayout,
  HomeRow,
  HomeSection,
  Lang,
  LanguagePref,
  Lyrics,
  MixPage,
  MixSummary,
  PlaybackSettings,
  PlayerState,
  PlaylistPage,
  PlaylistSummary,
  QueueView,
  RadioPage,
  SearchResults,
  SleepRequest,
  SleepState,
  StreamFormat,
  Theme,
  Track,
  TrackEntry,
  TrackInfo,
  UiSettings,
} from "../ipc";
import type { LibraryTab, Route, SettingsSection } from "../stores/nav.svelte";

interface Fixtures {
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
}

const found = import.meta.glob<Fixtures>("./fixtures.json", { eager: true, import: "default" });
const fx: Fixtures = Object.values(found)[0] ?? synthetic();
const params = new URLSearchParams(location.search);

const demoMix: MixSummary = {
  id: "0000000000000000000000000000ff",
  title: "My Mix 1",
  subtitle: "Artiste de démonstration et d'autres",
  imageUrl: null,
  mixType: "DAILY_MIX",
};
const mixPage: MixPage = fx.mix ?? { mix: demoMix, imageLargeUrl: null, tracks: fx.album.tracks.slice(0, 10) };
const radioPage: RadioPage = fx.radio ?? {
  seed: "track",
  id: fx.album.tracks[0]?.id ?? 1,
  title: fx.album.tracks[0]?.title ?? "Titre",
  imageUrl: fx.album.tracks[0]?.album.coverUrl ?? null,
  tracks: [...fx.album.tracks].reverse(),
};
const libraryMixes: MixSummary[] = fx.library.mixes ?? [mixPage.mix];

const allTracks = new Map<number, Track>();
for (const t of [
  ...fx.album.tracks,
  ...fx.artist.topTracks,
  ...fx.playlist.tracks,
  ...fx.search.tracks,
  ...fx.library.tracks,
  ...mixPage.tracks,
  ...radioPage.tracks,
]) {
  allTracks.set(t.id, t);
}

let context: Track[] = fx.album.tracks;
let index = Math.min(1, context.length - 1);
const manual: Track[] = [];
/** `?volume=60` ou `?volume=muted` : chaîne audio hors bit-perfect. */
const volumeParam = params.get("volume");
/** `?hiresedition=Thriller` : flux venu d'une autre édition hi-res (#18). */
const hiresEdition = params.get("hiresedition");
/** `?hiresoffer=Thriller` : réglage « Demander », bandeau qui propose une édition hi-res (#59). */
const hiresOfferParam = params.get("hiresoffer");
/** `?hirestwin=1` : la page album propose un album jumeau hi-res (#59). */
const hiresTwin = params.has("hirestwin");

/** `?sleep=30` (minuteur de 30 min) ou `?sleep=track` (fin du titre), #85. */
function mockSleep(param: string | null): SleepState | null {
  if (param === "track") return { kind: "endOfTrack" };
  const minutes = Number(param);
  return param && minutes > 0 ? { kind: "minutes", endsAt: Date.now() + minutes * 60_000 } : null;
}

function offerFormat(album: string): StreamFormat {
  return { quality: "HI_RES_LOSSLESS", codec: "flac", bitDepth: 24, sampleRate: 192000, bitrate: 5071, hiresEdition: album };
}

const state: PlayerState = {
  track: context[index] ?? null,
  stream: { quality: "HI_RES_LOSSLESS", codec: "flac", bitDepth: 24, sampleRate: 44100, bitrate: 1854, hiresEdition },
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
  hiresOffer: hiresOfferParam ? offerFormat(hiresOfferParam) : null,
  sleep: mockSleep(params.get("sleep")),
};

let ui: UiSettings = {
  theme: (params.get("theme") as Theme) ?? "seaglass",
  home: { order: [], hidden: [] },
  // La playlist des fixtures et les titres likés, pour voir la section « Épinglés ».
  pins: [
    { kind: "playlist", uuid: fx.playlist.playlist.uuid, title: fx.playlist.playlist.title },
    { kind: "likedTracks" },
  ],
  // `?accent=ff5c8a` : couleur d'accent personnalisée.
  accent: params.get("accent") ? `#${params.get("accent")}` : null,
  // `?lang=en` : interface en anglais.
  language: (params.get("lang") as LanguagePref) ?? "system",
};

/** « Système » du backend factice : la langue du navigateur. */
function effectiveLanguage(pref: LanguagePref): Lang {
  if (pref !== "system") return pref;
  return navigator.language.startsWith("fr") ? "fr" : "en";
}

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
let playback: PlaybackSettings = { cacheSize: 64, hiresUpgrade: hiresOfferParam ? "ask" : "auto" };
let autostart = params.has("autostart");

type Listener = (e: { payload: unknown }) => void;
const listeners = new Map<string, Set<Listener>>();

function emit(event: string, payload: unknown) {
  listeners.get(event)?.forEach((cb) => cb({ payload: structuredClone(payload) }));
}

function mockUpdate() {
  return params.has("update")
    ? {
        version: "0.5.0",
        currentVersion: "0.4.4",
        notes: ["- Installeur Windows", "- Mises à jour automatiques"].join("\n"),
      }
    : null;
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
    hiresOffer:
      !track.hires && hiresOfferParam !== null && playback.hiresUpgrade === "ask" ? offerFormat(hiresOfferParam) : null,
    // Avec `?hiresedition=…` et le réglage actif, un titre non hi-res est « remplacé » par
    // une autre édition, comme le ferait le backend.
    ...(() => {
      const upgraded = !track.hires && hiresEdition !== null && playback.hiresUpgrade === "auto";
      const hires = track.hires || upgraded;
      return {
        stream: hires
          ? {
              quality: "HI_RES_LOSSLESS",
              codec: "flac",
              bitDepth: 24,
              sampleRate: 48000,
              bitrate: 2016,
              hiresEdition: upgraded ? hiresEdition : null,
            }
          : { quality: "LOSSLESS", codec: "flac", bitDepth: 16, sampleRate: 44100, bitrate: 893, hiresEdition: null },
        output: { format: hires ? "s32" : "s16", samplerate: hires ? 48000 : 44100, channelCount: 2 },
      };
    })(),
  });
  state.hasNext = manual.length > 0 || index + 1 < context.length;
  emit("player-state", state);
  emit("queue-state", queueView());
}

// Écritures (#4) : likes et playlists de l'utilisateur, gardés en mémoire.
const liked = new Set<number>(fx.library.tracks.map((t) => t.id));
const own: PlaylistSummary[] = fx.library.playlists.filter((p) => p.mine);
if (params.has("mine") && !own.some((p) => p.uuid === fx.playlist.playlist.uuid)) {
  own.unshift({ ...fx.playlist.playlist, mine: true, editorial: false });
}
const ownPages = new Map<string, PlaylistPage>();

/** Page d'une de vos playlists, créée au premier accès avec les titres de démonstration. */
function ownPage(uuid: string): PlaylistPage | undefined {
  let page = ownPages.get(uuid);
  if (!page) {
    const summary = own.find((p) => p.uuid === uuid);
    if (!summary) return undefined;
    page = { playlist: summary, tracks: fx.playlist.tracks.map((t) => ({ ...t })) };
    ownPages.set(uuid, page);
  }
  page.playlist.numberOfTracks = page.tracks.length;
  return page;
}

function addToOwn(uuid: string, trackIds: number[]): number {
  const page = ownPage(uuid);
  if (!page) throw new Error("playlist introuvable");
  let added = 0;
  for (const id of trackIds) {
    const t = allTracks.get(id);
    if (!t || page.tracks.some((e) => e.id === id)) continue;
    page.tracks.push({ ...t, addedAt: Date.now() });
    added++;
  }
  page.playlist.numberOfTracks = page.tracks.length;
  return added;
}

const handlers: Record<string, (args: Record<string, unknown>) => unknown> = {
  auth_status: () => (params.has("loggedout") ? null : { userId: 1, countryCode: "CH" }),
  login: () => ({ userId: 1, countryCode: "CH" }),
  logout: () => null,
  home: () => arrangeHome(fx.home, ui.home),
  feed: () => fx.feed ?? [],
  lyrics: ({ trackId }) => mockLyrics(Number(trackId)),
  track_info: ({ trackId }) => mockTrackInfo(Number(trackId)),
  search: ({ query }) => (String(query).trim() ? fx.search : { tracks: [], albums: [], artists: [], playlists: [] }),
  // Avec `?hirestwin=1`, l'album affiché n'est pas hi-res et a un jumeau qui l'est.
  album: () => (hiresTwin ? { ...fx.album, album: { ...fx.album.album, hires: false } } : fx.album),
  hires_twin_album: () =>
    hiresTwin ? { ...fx.album.album, id: fx.album.album.id + 1, title: `${fx.album.album.title} (Hi-Res)`, hires: true } : null,
  artist: () => fx.artist,
  playlist: ({ uuid }) => ownPage(String(uuid)) ?? fx.playlist,
  liked_tracks: () => [...liked],
  set_track_liked: ({ trackId, liked: on }) => {
    if (on) liked.add(Number(trackId));
    else liked.delete(Number(trackId));
    emit("liked-tracks", [...liked]);
  },
  own_playlists: () => own.map((p) => ownPage(p.uuid)?.playlist ?? p),
  add_to_playlist: ({ uuid, trackIds }) => addToOwn(String(uuid), [...(trackIds as number[])]),
  create_playlist: ({ title, trackIds }) => {
    const uuid = crypto.randomUUID();
    const summary: PlaylistSummary = {
      uuid,
      title: String(title).trim(),
      description: null,
      imageUrl: null,
      numberOfTracks: 0,
      duration: null,
      editorial: false,
      mine: true,
    };
    own.unshift(summary);
    ownPages.set(uuid, { playlist: summary, tracks: [] });
    addToOwn(uuid, [...(trackIds as number[])]);
    return uuid;
  },
  remove_from_playlist: ({ uuid, index, trackId }) => {
    const page = ownPage(String(uuid));
    if (page?.tracks[Number(index)]?.id !== Number(trackId)) {
      throw new Error("la playlist a changé, rouvrez-la avant de retirer ce titre");
    }
    page.tracks.splice(Number(index), 1);
    page.playlist.numberOfTracks = page.tracks.length;
  },
  mix: () => mixPage,
  // La radio factice garde ses titres, mais reprend le point de départ demandé.
  radio: ({ seed, id }) => ({ ...radioPage, seed, id }),
  library_mixes: () => libraryMixes,
  library_tracks: () => fx.library.tracks,
  library_albums: () => fx.library.albums,
  library_artists: () => fx.library.artists,
  library_playlists: () => fx.library.playlists,
  player_state: () => state,
  queue_state: () => queueView(),
  ui_settings: () => ui,
  // `?update=1` : simule une mise à jour prête.
  update_status: () => mockUpdate(),
  install_update: () => null,
  // Sans `?update=1`, l'app est « à jour ».
  check_for_updates: () => {
    const u = mockUpdate();
    if (u) emit("update-ready", u);
    return u;
  },
  set_ui_settings: ({ settings }) => {
    ui = settings as UiSettings;
  },
  language: () => effectiveLanguage(ui.language),
  set_language: ({ language }) => {
    ui.language = language as LanguagePref;
    return effectiveLanguage(ui.language);
  },
  set_home_layout: ({ sections }) => {
    const list = sections as { key: string; hidden: boolean }[];
    ui.home = { order: list.map((s) => s.key), hidden: list.filter((s) => s.hidden).map((s) => s.key) };
    return ui.home;
  },
  reset_home_layout: () => (ui.home = { order: [], hidden: [] }),
  // Copie JSON : l'UI envoie des proxys Svelte, que `structuredClone` refuse.
  set_pins: ({ pins }) => (ui.pins = JSON.parse(JSON.stringify(pins))),
  playback_settings: () => playback,
  // Même validation que le backend : seules les tailles proposées sont acceptées.
  set_playback_settings: ({ settings }) => {
    const next = settings as PlaybackSettings;
    if (![32, 64, 128, 256].includes(next.cacheSize)) {
      throw new Error(`taille de cache non prise en charge : ${next.cacheSize} Mio`);
    }
    playback = next;
  },
  autostart_enabled: () => autostart,
  set_autostart: ({ enabled }) => {
    autostart = Boolean(enabled);
    return autostart;
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
  answer_hires_offer: ({ accept }) => {
    if (!state.hiresOffer) return;
    if (accept) state.stream = state.hiresOffer;
    state.hiresOffer = null;
    emit("player-state", state);
  },
  set_sleep_timer: ({ timer }) => {
    const req = timer as SleepRequest | null;
    if (req?.kind === "minutes" && (req.minutes < 1 || req.minutes > 600)) {
      throw new Error(`durée du minuteur invalide : ${req.minutes} min (1 à 600)`);
    }
    state.sleep = !req
      ? null
      : req.kind === "minutes"
        ? { kind: "minutes", endsAt: Date.now() + req.minutes * 60_000 }
        : { kind: "endOfTrack" };
    emit("player-state", state);
  },
  clear_queue: () => {
    manual.length = 0;
    context = context.slice(index, index + 1);
    index = 0;
    state.hasNext = false;
    emit("player-state", state);
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

/** Couplets inventés, répartis sur toute la durée du titre (aucune vraie parole). */
const LYRIC_VERSES = [
  ["Le verre poli par la marée", "garde la couleur des étés", "on le ramasse sans y penser", "il brille encore au soleil levé"],
  ["", "Refrain, les vagues reviennent", "et la lumière se souvient", "de tout ce qu'on n'a pas dit", "sur le sable, à minuit"],
  ["Une bouteille il y a cent ans", "jetée au large par le vent", "", "elle revient en éclats lents", "plus douce qu'au commencement"],
];

function mockLyrics(trackId: number): Lyrics | null {
  const mode = params.get("lyrics");
  if (mode === "none") return null;
  const lines = [...LYRIC_VERSES, LYRIC_VERSES[1]].flat();
  if (mode === "plain") {
    return { trackId, provider: "MUSIXMATCH", rtl: false, text: { kind: "plain", lines } };
  }
  const duration = allTracks.get(trackId)?.duration ?? 200;
  const step = (duration * 1000 - 8000) / lines.length;
  return {
    trackId,
    provider: "MUSIXMATCH",
    rtl: false,
    text: {
      kind: "synced",
      lines: lines.map((text, i) => ({ timeMs: Math.round(6000 + i * step), text })),
    },
  };
}

/** Infos factices (#3) : titre hi-res en 24/48, sinon 16/44,1 avec l'édition `?hiresedition=`. */
function mockTrackInfo(trackId: number): TrackInfo {
  const track = allTracks.get(trackId);
  if (!track) throw new Error(`titre ${trackId} introuvable`);
  const cd: StreamFormat = {
    quality: "LOSSLESS",
    codec: "flac",
    bitDepth: 16,
    sampleRate: 44100,
    bitrate: 893,
    hiresEdition: null,
  };
  const hires: StreamFormat = { ...cd, quality: "HI_RES_LOSSLESS", bitDepth: 24, sampleRate: 48000, bitrate: 2016 };
  return {
    track,
    tags: track.hires ? ["LOSSLESS", "HIRES_LOSSLESS"] : ["LOSSLESS"],
    stream: track.hires ? hires : cd,
    streamError: null,
    trackGain: -6.17,
    trackPeak: 0.997284,
    albumGain: -8.25,
    albumPeak: 0.997253,
    bpm: 147,
    key: track.hires ? "FSharp" : "Bb",
    keyScale: "MINOR",
    copyright: "℗ 2024 Label de démonstration",
    trackNumber: Math.max(1, fx.album.tracks.findIndex((t) => t.id === trackId) + 1),
    volumeNumber: 1,
    hiresEdition:
      !track.hires && hiresEdition !== null
        ? {
            track: { ...track, id: track.id + 1_000_000, hires: true, album: { ...track.album, title: hiresEdition } },
            stream: hires,
            played: playback.hiresUpgrade === "auto",
          }
        : null,
  };
}

/** Paramètres d'URL de navigation, appliqués une fois l'app montée. */
export function initialView(): {
  route: Route | null;
  nowPlaying: boolean;
  queue: boolean;
  lyrics: boolean;
  trackInfo: boolean;
} {
  const routes: Record<string, Route> = {
    album: { name: "album", id: fx.album.album.id },
    artist: { name: "artist", id: fx.artist.artist.id },
    playlist: { name: "playlist", uuid: fx.playlist.playlist.uuid },
    mix: { name: "mix", id: mixPage.mix.id },
    radio: { name: "radio", seed: radioPage.seed, id: radioPage.id },
    library: { name: "library", tab: (params.get("tab") as LibraryTab) ?? "playlists" },
    search: { name: "search" },
    feed: { name: "feed" },
    settings: { name: "settings", section: (params.get("tab") as SettingsSection) ?? "appearance" },
  };
  return {
    route: routes[params.get("route") ?? ""] ?? null,
    nowPlaying: params.has("nowplaying"),
    queue: params.has("queue"),
    lyrics: params.has("lyrics"),
    trackInfo: params.has("trackinfo"),
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
    home: [
      { key: "FOR_YOU", title: "Pour vous", items: [{ kind: "album", ...album }] },
      {
        key: "HISTORY_MIXES",
        title: "Votre historique de lecture",
        items: [
          {
            kind: "mix",
            id: "0000000000000000000000000000ff",
            title: "My Mix 1",
            subtitle: "Artiste de démonstration et d'autres",
            imageUrl: null,
            mixType: "DAILY_MIX",
          },
        ],
      },
    ],
    feed: [
      { occurredAt: Date.now(), seen: false, item: { kind: "album", ...album, albumType: "SINGLE" } },
      {
        occurredAt: Date.UTC(2024, 0, 1),
        seen: true,
        item: {
          kind: "mix",
          id: "0000000000000000000000000000fe",
          title: "December 2023",
          subtitle: "Artiste de démonstration et plus",
          imageUrl: null,
          mixType: "HISTORY_MONTHLY_MIX",
        },
      },
      { occurredAt: Date.UTC(2024, 0, 1), seen: true, item: { kind: "album", ...album } },
    ],
    library: { tracks: likedTracks, albums: [album], artists: [], playlists: [] },
  };
}
