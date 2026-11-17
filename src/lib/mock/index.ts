// Backend factice pour prévisualiser l'UI dans un navigateur (`pnpm dev`, hors Tauri).
// Données : `fixtures.json`, généré par `cargo test dump_ui_fixtures -- --ignored`
// (gitignoré). Sans ce fichier, un jeu minimal synthétique est utilisé.
//
// Paramètres d'URL : `?theme=studio|atelier|seaglass`, `?route=album|artist|playlist|
// mix|radio|library|stats|recap|search|home|feed|settings`, `?tab=` (onglet de la bibliothèque ou section des
// réglages : appearance|playback|app), `?nowplaying=1`, `?paused=1`, `?loggedout=1`,
// `?queue=1`, `?update=1`, `?autostart=1`, `?tray=1`, `?volume=60|muted`, `?accent=ff5c8a`,
// `?platform=android` (mise en page tactile, à regarder en largeur de téléphone ; avec `&drive=1`,
// mode conduite ouvert, #193),
// `?lyrics=1|plain|none` (ouvre le panneau Paroles ; texte simple ou aucune parole ; avec `?queue=1`,
// paroles et file côte à côte en fenêtre large),
// `?mine=1` (la playlist de démonstration devient la vôtre : retrait de titres possible),
// `?trackinfo=1` (ouvre les infos du titre en cours), `?autoradio=1` (la suite de la file est
// une radio automatique, réglage activé), `?long=5000` (playlist de démonstration et titres likés
// remplacés par n titres synthétiques, 10 000 au plus), `?ticks=1` (`player-state` émis à 4 Hz
// pendant la lecture, comme le vrai backend) : mesure des longues listes (#88), `?sleep=30|track|context`
// (minuteur de mise en veille actif), `?device=dac|missing` (périphérique de sortie choisi, branché ou
// débranché), `?normalization=track|album` (normalisation du volume
// active), `?exclusive=1` (mode exclusif WASAPI, #245), `?slow=<ms>` (ouverture d'une playlist ralentie : « Chargement… » après 300 ms,
// copie gardée affichée aussitôt à la réouverture, #152).

import { blindTestMock } from "./blindTest";
import type {
  PlaySource,
  AlbumPage,
  AlbumSummary,
  ArtistPage,
  ArtistSummary,
  BlockTarget,
  Blocked,
  CleanupReport,
  FeedItem,
  HomeLayout,
  HomeRow,
  HomeSection,
  Lang,
  LanguagePref,
  LaterItem,
  LibraryList,
  LikedIds,
  LikeTarget,
  ListOp,
  ListOpsResult,
  Lyrics,
  MixPage,
  MixSummary,
  AudioDevices,
  OutputDevice,
  PlaybackSettings,
  PlayerState,
  PlaylistPage,
  PlaylistFolder,
  PlaylistSummary,
  QueueView,
  RadioPage,
  SearchResults,
  SleepRequest,
  SleepState,
  SmartList,
  StreamFormat,
  Theme,
  Track,
  TrackEntry,
  TrackInfo,
  AlbumCreditGroup,
  TrackSort,
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
let pinShortcutParam = params.has("pinshortcut");

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
/** `?autoradio=1` : la suite de la file est la radio du titre en cours (#78). */
let radioSeed: Track | null = null;
if (params.has("autoradio") && context[index]) {
  radioSeed = context[index];
  context = [radioSeed, ...radioPage.tracks.filter((t) => t.id !== radioSeed?.id)];
  index = 0;
}
/** « Joué depuis » (#124) : l'album de démonstration, sauf sur le titre de départ d'une radio. */
let playSource: PlaySource | null = radioSeed
  ? null
  : { kind: "album", id: fx.album.album.id, title: fx.album.album.title };
/** `?manual=1` : quelques titres dans la file manuelle, pour la réorganiser (#80). */
const manual: Track[] = params.has("manual") ? fx.playlist.tracks.slice(0, 4) : [];
/** File d'avant le dernier vidage ou retrait (#349), comme `Player::undo_queue`. */
let queueUndo: {
  manual: Track[];
  context: Track[];
  index: number;
  radioSeed: Track | null;
  playSource: PlaySource | null;
} | null = null;
/** `?volume=60` ou `?volume=muted` : chaîne audio hors bit-perfect. */
const volumeParam = params.get("volume");
/** `?hiresedition=Thriller` : flux venu d'une autre édition hi-res (#18), album ouvrable (#111). */
const hiresEditionParam = params.get("hiresedition");
const hiresEdition = hiresEditionParam === null ? null : { id: fx.album.album.id, title: hiresEditionParam };
/** `?hiresoffer=Thriller` : réglage « Demander », bandeau qui propose une édition hi-res (#59). */
const hiresOfferParam = params.get("hiresoffer");
/** `?hirestwin=1` : la page album propose un album jumeau hi-res (#59). */
const hiresTwin = params.has("hirestwin");

/** Cache en mémoire (#453) : `?cached=whole` (titre entier), `?cached=40` (40 s d'avance), 90 s
 *  d'avance par défaut. */
function mockCached(position: number, duration: number): number {
  const param = params.get("cached");
  const ahead = param === "whole" ? Infinity : Number(param ?? 90) || 0;
  return Math.min(position + ahead, duration);
}

/** `?sleep=30` (minuteur de 30 min), `?sleep=track` (fin du titre, #85) ou `?sleep=context` (fin de la file, #436). */
function mockSleep(param: string | null): SleepState | null {
  if (param === "track") return { kind: "endOfTrack" };
  if (param === "context") return { kind: "endOfContext" };
  const minutes = Number(param);
  return param && minutes > 0 ? { kind: "minutes", endsAt: Date.now() + minutes * 60_000 } : null;
}

function offerFormat(album: string): StreamFormat {
  const hiresEdition = { id: fx.album.album.id, title: album };
  return { quality: "HI_RES_LOSSLESS", codec: "flac", bitDepth: 24, sampleRate: 192000, bitrate: 5071, hiresEdition };
}

const state: PlayerState = {
  // `?idle=1` : rien en lecture (vue « En lecture » vide, #399).
  track: params.has("idle") ? null : (context[index] ?? null),
  stream: { quality: "HI_RES_LOSSLESS", codec: "flac", bitDepth: 24, sampleRate: 44100, bitrate: 1854, hiresEdition },
  output: { format: "s32", samplerate: 44100, channelCount: 2 },
  // Calculé plus bas, selon le mode exclusif factice.
  deviceFormat: null,
  outputDriver: "wasapi",
  exclusive: false,
  // Calculé plus bas, après les réglages de lecture factices.
  outputDevice: null,
  loading: false,
  paused: params.has("paused"),
  buffering: false,
  idle: params.has("idle"),
  timePos: 48,
  duration: context[index]?.duration ?? 0,
  cached: mockCached(48, context[index]?.duration ?? 0),
  nextReady: params.has("nextready"),
  shuffle: false,
  repeat: "off",
  hasPrevious: true,
  hasNext: true,
  volume: {
    level: volumeParam && volumeParam !== "muted" ? Math.min(100, Math.max(0, Number(volumeParam) || 0)) : 100,
    muted: volumeParam === "muted",
  },
  hiresOffer: hiresOfferParam ? offerFormat(hiresOfferParam) : null,
  // Calculé plus bas, après les réglages de lecture factices.
  normalization: null,
  sleep: mockSleep(params.get("sleep")),
  /** `?mpv=cpu`, `?mpv=stopped` ou `?mpv=missing` : bandeau « mpv ne tourne pas » (#92, #261). */
  // `?ended=1` : file terminée (carton « Fin » du thème Séance, #399). Bande originale :
  // vraie pour l'album d'Interstellar des fixtures, comme la liste du backend.
  soundtrack: context[index]?.album.id === 156749593,
  ended: params.has("ended"),
  blindTest: false,
  mpvIssue: ({ cpu: "cpuUnsupported", stopped: "stopped", missing: "missing" } as const)[params.get("mpv") ?? ""] ?? null,
};

let ui: UiSettings = {
  theme: (params.get("theme") as Theme) ?? "seaglass",
  home: { order: [], hidden: [], featuredHidden: false },
  // La playlist des fixtures et les titres likés, pour voir la section « Épinglés ».
  pins: [
    { kind: "playlist", uuid: fx.playlist.playlist.uuid, title: fx.playlist.playlist.title },
    { kind: "likedTracks" },
  ],
  // `?accent=ff5c8a` : couleur d'accent personnalisée du thème affiché.
  accents: params.get("accent") ? { [(params.get("theme") as Theme) ?? "seaglass"]: `#${params.get("accent")}` } : {},
  // `?favorites=alloy,terminal` : thèmes favoris (#390).
  favoriteThemes: (params.get("favorites")?.split(",") ?? []) as Theme[],
  // `?lang=en` : interface en anglais.
  language: (params.get("lang") as LanguagePref) ?? "system",
  // `?tray=1` : fermer la fenêtre la réduit dans la zone de notification (#83).
  closeAction: params.has("tray") ? "tray" : "quit",
  // `?discord=1` : présence Discord activée (#215).
  discord: params.has("discord"),
  sorts: [],
  zoom: 100,
  // `?motion=reduced|full` : animations réduites ou forcées (#351).
  motion: (params.get("motion") as UiSettings["motion"]) ?? "system",
  // `?eggs=off` : clins d'œil désactivés (#399) ; `?date=` et `?egg=` dans `lib/eggs/`.
  easterEggs: params.get("eggs") !== "off",
  // `?explicit=hide` : contenu explicite masqué (#368) ; `?ai=show` : musique générée par
  // IA affichée (#369). Le backend factice ne filtre pas.
  content: { hideExplicit: params.get("explicit") === "hide", hideAi: params.get("ai") !== "show" },
  // `?hotkeys=1` : raccourcis globaux activés (#413) ; `?hotkeys=taken` : le premier est pris.
  globalShortcuts: {
    enabled: params.has("hotkeys"),
    bindings: {
      playPause: "Ctrl+Alt+Space",
      next: "Ctrl+Alt+ArrowRight",
      previous: "Ctrl+Alt+ArrowLeft",
      volumeUp: "Ctrl+Alt+ArrowUp",
      volumeDown: "Ctrl+Alt+ArrowDown",
      like: "Ctrl+Alt+KeyL",
      showWindow: "Ctrl+Alt+KeyS",
    },
  },
};

/** « Système » du backend factice : la langue du navigateur. */
function effectiveLanguage(pref: LanguagePref): Lang {
  if (pref !== "system") return pref;
  return navigator.language.startsWith("fr") ? "fr" : "en";
}

/** Version simplifiée de `HomeLayout::arrange` : sections connues dans l'ordre enregistré,
 *  les autres à leur place dans le flux. Le flux factice ne change pas. */
/** Historique d'écoute factice (#202) : quelques albums des fixtures. */
let playedAlbums: AlbumSummary[] = fx.search.albums.slice(0, 5);

/** « Écoutés dans Seaglass » en tête du flux, comme le vrai backend. */
function withPlayed(rows: HomeRow[]): HomeRow[] {
  if (playedAlbums.length === 0) return rows;
  const items = playedAlbums.map((a) => ({ kind: "album" as const, ...a }));
  return [{ key: "SEAGLASS_RECENT", title: "Écoutés dans Seaglass", items }, ...rows];
}

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
// `?normalization=track|album` : normalisation active (gain factice de −6,5 dB).
const normalizationParam = params.get("normalization") as PlaybackSettings["normalization"] | null;
// `?device=missing` : le périphérique choisi est débranché ; `?device=dac` : il est branché.
const DAC = { id: "wasapi/{27264a86-c734-4c85-876c-dcfeb567529f}", name: "Haut-parleurs (Focusrite USB Audio)" };
const SPEAKERS = { id: "wasapi/{0d7e4c2b-58a1-4c5e-9f1e-3b2a9c1d7e44}", name: "Haut-parleurs (Realtek(R) Audio)" };
const deviceParam = params.get("device");
let playback: PlaybackSettings = {
  cacheSize: 64,
  hiresUpgrade: hiresOfferParam ? "ask" : "auto",
  audioDevice: deviceParam ? DAC : null,
  autoRadio: radioSeed !== null,
  pauseOnUnplug: true,
  resumeOnLaunch: false,
  deviceProfiles: params.has("profiles"),
  wifiQuality: "lossless",
  cellularQuality: "lossless",
  lowBatteryQuality: false,
  normalization: normalizationParam ?? "off",
  exclusive: params.has("exclusive"),
};
const audioDevices: AudioDevices = {
  devices: deviceParam === "missing" ? [SPEAKERS] : [SPEAKERS, DAC],
  systemDefault: SPEAKERS.id,
};

/** Même logique que `player/output.rs` : choix s'il est branché, sinon défaut du système. */
function outputDevice(): OutputDevice {
  const chosen = playback.audioDevice;
  const present = chosen && audioDevices.devices.some((d) => d.id === chosen.id);
  if (chosen && present) return { name: chosen.name, systemDefault: false, missing: null };
  return { name: SPEAKERS.name, systemDefault: true, missing: chosen ? chosen.name : null };
}
state.outputDevice = outputDevice();
/** Gain factice : −6,5 dB par titre, −4 dB par album. */
function mockGain(): number | null {
  return playback.normalization === "off" ? null : playback.normalization === "track" ? -6.5 : -4;
}
state.normalization = mockGain();
/** Comme `player/exclusive.rs` et mpv : en exclusif, le format du flux part tel quel et la
 *  normalisation est suspendue ; en partagé, Windows impose son format de mixage. */
function applyExclusive() {
  state.exclusive = playback.exclusive;
  if (playback.exclusive) state.normalization = null;
  state.deviceFormat = !state.output
    ? null
    : playback.exclusive
      ? { ...state.output }
      : { format: "float", samplerate: 48000, channelCount: state.output.channelCount };
}
applyExclusive();
let autostart = params.has("autostart");
let fullscreen = params.has("fullscreen");
let miniOnTop = true;

type Listener = (e: { payload: unknown }) => void;
const listeners = new Map<string, Set<Listener>>();

function emit(event: string, payload: unknown) {
  listeners.get(event)?.forEach((cb) => cb({ payload: structuredClone(payload) }));
}

const blind = blindTestMock({ state, track: (id) => allTracks.get(id), emit });

let mockDownloaded = params.get("update") !== "download";

function mockUpdate() {
  return params.has("update")
    ? {
        version: "0.5.0",
        currentVersion: "0.4.4",
        notes: ["- Installeur Windows", "- Mises à jour automatiques"].join("\n"),
        // Android : proposée avant d'être téléchargée, avec la taille de l'APK.
        ...(params.get("platform") === "android" ? { size: 41_200_000 } : {}),
      }
    : null;
}

function queueView(): QueueView {
  return {
    manual: [...manual],
    upcoming: context.slice(index + 1, index + 51),
    radio: radioSeed,
    source: playSource,
  };
}

function setCurrent(track: Track | undefined) {
  if (!track) return;
  Object.assign(state, {
    track,
    timePos: 0,
    duration: track.duration,
    cached: mockCached(0, track.duration),
    nextReady: false,
    paused: false,
    idle: false,
    soundtrack: track.album.id === 156749593,
    ended: false,
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
  applyExclusive();
  emit("player-state", state);
  emit("queue-state", queueView());
}

// Écritures (#4, #81) : favoris et playlists de l'utilisateur, gardés en mémoire.
// `?truncated=1` : chaque catégorie se présente comme coupée au plafond (#91), avec un
// total annoncé de 12 345 (`?truncated=nototal` : total inconnu).
function libraryList<T>(items: T[]): LibraryList<T> {
  const mode = params.get("truncated");
  if (!mode) return { items, truncated: false, total: items.length };
  return { items, truncated: true, total: mode === "nototal" ? null : 12345 };
}

const liked = new Set<number>(fx.library.tracks.map((t) => t.id));
const favAlbums = [...(fx.library.albums as AlbumSummary[])];
const favArtists = [...(fx.library.artists as ArtistSummary[])];
const libPlaylists = [...fx.library.playlists];

// Dossiers de playlists (#331) : aucun par défaut (rien ne doit s'afficher), un dossier de
// démonstration avec `?folders=1`.
let mockFolders: PlaylistFolder[] =
  params.get("folders") === "1"
    ? [{ id: "f0000000-0000-4000-8000-000000000001", name: "Soirées", playlists: libPlaylists.slice(0, 2).map((p) => p.uuid) }]
    : [];

function unfile(uuids: string[]) {
  mockFolders = mockFolders.map((f) => ({ ...f, playlists: f.playlists.filter((u) => !uuids.includes(u)) }));
}

function likedIds(): LikedIds {
  return {
    tracks: [...liked],
    albums: favAlbums.map((a) => a.id),
    artists: favArtists.map((a) => a.id),
    playlists: libPlaylists.filter((p) => !p.mine).map((p) => p.uuid),
    mixes: libraryMixes.map((m) => m.id),
  };
}

/** Ajoute en tête (comme le tri par date du service) ou retire un favori de la liste. */
function toggleIn<T>(list: T[], known: T[], match: (x: T) => boolean, on: boolean) {
  const i = list.findIndex(match);
  if (i >= 0 && !on) list.splice(i, 1);
  const found = known.find(match);
  if (i < 0 && on && found) list.unshift(found);
}

function setLiked(target: LikeTarget, on: boolean) {
  switch (target.kind) {
    case "track":
      if (on) liked.add(Number(target.id));
      else liked.delete(Number(target.id));
      break;
    case "album": {
      const known = [fx.album.album, ...fx.search.albums, ...fx.artist.albums, ...fx.artist.singles];
      toggleIn(favAlbums, known, (a) => a.id === Number(target.id), on);
      break;
    }
    case "artist":
      toggleIn(favArtists, [fx.artist.artist, ...fx.search.artists], (a) => a.id === Number(target.id), on);
      break;
    case "playlist":
      toggleIn(libPlaylists, [fx.playlist.playlist, ...fx.search.playlists], (p) => p.uuid === target.uuid, on);
      break;
    case "mix": {
      // Ajouté en fin de liste, comme le service (#204).
      const i = libraryMixes.findIndex((m) => m.id === target.id);
      if (i >= 0 && !on) libraryMixes.splice(i, 1);
      if (i < 0 && on) libraryMixes.push(mixPage.mix.id === target.id ? mixPage.mix : { ...mixPage.mix, id: target.id });
      break;
    }
  }
  emit("likes", likedIds());
}
// « Plus tard » (#196) : deux titres et un album de démonstration, plus récent en tête.
const laterItems: LaterItem[] = [
  ...fx.library.tracks.slice(0, 2).map((t) => ({ kind: "track" as const, ...t })),
  { kind: "album" as const, ...fx.album.album },
];

function laterIds(): LikedIds {
  const ids: LikedIds = { tracks: [], albums: [], artists: [], playlists: [], mixes: [] };
  for (const item of laterItems) {
    if (item.kind === "track") ids.tracks.push(item.id);
    else if (item.kind === "album") ids.albums.push(item.id);
    else if (item.kind === "artist") ids.artists.push(item.id);
    else ids.playlists.push(item.uuid);
  }
  return ids;
}

function laterKey(item: LaterItem | LikeTarget): string {
  return item.kind === "playlist" ? `playlist:${item.uuid}` : `${item.kind}:${item.id}`;
}

function setSavedForLater(target: LikeTarget, on: boolean) {
  const i = laterItems.findIndex((item) => laterKey(item) === laterKey(target));
  if (i >= 0 && !on) laterItems.splice(i, 1);
  if (i < 0 && on) {
    const known: LaterItem[] = [
      ...fx.library.tracks.map((t) => ({ kind: "track" as const, ...t })),
      ...[fx.album.album, ...fx.search.albums].map((a) => ({ kind: "album" as const, ...a })),
      ...[fx.artist.artist, ...fx.search.artists].map((a) => ({ kind: "artist" as const, ...a })),
      ...[fx.playlist.playlist, ...fx.search.playlists].map((p) => ({ kind: "playlist" as const, ...p })),
    ];
    const found = known.find((item) => laterKey(item) === laterKey(target));
    if (found) laterItems.unshift(found);
  }
  emit("later", laterIds());
}

// Blocages (#334) : un titre et un artiste de démonstration, plus récent en tête.
const blocked: Blocked = {
  tracks: fx.search.tracks.slice(-1).map((t) => ({ ...t })),
  artists: fx.search.artists.slice(-1).map((a) => ({ ...a })),
};

function setBlocked(target: BlockTarget, on: boolean) {
  if (target.kind === "track") {
    const id = Number(target.id);
    const known = [...fx.search.tracks, ...fx.album.tracks, ...fx.library.tracks];
    toggleIn(blocked.tracks, known, (t) => t.id === id, on);
  } else {
    const known = [fx.artist.artist, ...fx.search.artists];
    toggleIn(blocked.artists, known, (a) => a.id === Number(target.id), on);
  }
}

const own: PlaylistSummary[] = fx.library.playlists.filter((p) => p.mine);
if (params.has("mine") && !own.some((p) => p.uuid === fx.playlist.playlist.uuid)) {
  own.unshift({ ...fx.playlist.playlist, mine: true, editorial: false });
}
const ownPages = new Map<string, PlaylistPage>();

/** Rapport factice de « Nettoyer » (#382) : un indisponible remplacé, un doublon d'édition,
 *  un indisponible sans remplaçant (non coché), après un court délai (`invoke`). */
function cleanupReport(tracks: Track[]): CleanupReport {
  const [a, b, c, d, e] = tracks;
  return a && b && c && d && e
      ? {
          scanned: tracks.length,
          items: [
            {
              kind: "unavailable",
              index: 1,
              track: b,
              replacement: e,
              suggested: true,
              op: { op: "replace", index: 1, trackId: b.id, with: e.id },
            },
            {
              kind: "duplicate",
              index: 2,
              track: c,
              keptIndex: 0,
              kept: a,
              sameTrack: false,
              suggested: true,
              op: { op: "remove", index: 2, trackId: c.id },
            },
            {
              kind: "unavailable",
              index: 3,
              track: d,
              replacement: null,
              suggested: false,
              op: { op: "remove", index: 3, trackId: d.id },
            },
          ],
        }
      : { scanned: tracks.length, items: [] };
}

/** Rien n'est modifié : tout est annoncé appliqué, et annulable (un titre « ne revient pas »). */
function cleanupResult(ops: ListOp[], undoing: boolean): ListOpsResult {
  return { applied: ops.length, skipped: undoing ? 1 : 0, undo: undoing ? [] : [...ops].reverse(), error: null };
}

/** Page d'une de vos playlists, créée au premier accès avec les titres de démonstration. */
function ownPage(uuid: string): PlaylistPage | undefined {
  let page = ownPages.get(uuid);
  if (!page) {
    const summary = own.find((p) => p.uuid === uuid);
    if (!summary) return undefined;
    page = { playlist: summary, tracks: fx.playlist.tracks.map((t) => ({ ...t })), revision: null };
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

/** `?long=<n>` : n titres synthétiques (les titres connus répétés, identifiants uniques). */
const longCount = Math.min(10_000, Math.max(0, Number(params.get("long")) || 0));
const longTracks: TrackEntry[] = (() => {
  const base = [...allTracks.values()];
  if (!longCount || base.length === 0) return [];
  const now = Date.now();
  return Array.from({ length: longCount }, (_, i) => {
    const t = base[i % base.length];
    // Le premier passage garde les vrais identifiants : le titre en cours y figure.
    const entry = { ...t, id: i < base.length ? t.id : t.id * 100_000 + i, addedAt: now - i * 3_600_000 };
    allTracks.set(entry.id, entry);
    return entry;
  });
})();
const longPlaylist: PlaylistPage | null = longCount
  ? { playlist: { ...fx.playlist.playlist, numberOfTracks: longCount }, tracks: longTracks, revision: null }
  : null;

const cachedPlaylists = new Map<string, PlaylistPage>();
/** Dernier titre retiré d'une playlist, pour l'annulation (#349). */
let removedFromPlaylist: PlaylistPage["tracks"][number] | null = null;
let removedMany: PlaylistPage["tracks"][number][] = [];
const homeFirstPage = Math.ceil(fx.home.length / 2);

/** `?slow=<ms>` : latence de la commande `playlist`, pour voir « Chargement… » (#152). */
const slowPlaylist = Math.max(0, Number(params.get("slow")) || 0);

/** Page actuelle, avec une révision tirée de ses titres (l'ETag du vrai backend). */
function currentPlaylist(uuid: string): PlaylistPage {
  const page = ownPage(uuid) ?? longPlaylist ?? fx.playlist;
  let hash = 5381;
  for (const t of page.tracks) hash = (Math.imul(hash, 33) ^ t.id) >>> 0;
  return { ...page, revision: `"${hash}-${page.tracks.length}"` };
}

/** 250 ms de lecture : `?ticks=1`, ou appelée directement par une mesure (les minuteries d'une
 *  page masquée sont ralenties). */
export function playbackTick() {
  if (state.paused || state.idle || state.loading || !(state.track || state.blindTest)) return;
  state.timePos = Math.min(state.timePos + 0.25, state.duration);
  blind.tick();
  emit("player-state", state);
}
if (params.has("ticks")) setInterval(playbackTick, 250);

let searchHistory: string[] = ["Daft Punk", "Moon Safari", "Bohemian Rhapsody"];

// Jam (#25) : `?jam=1` l'affiche démarrée en ligne, `?jam=local` sur le réseau local,
// `?jam=reconnecting` en reconnexion ; avec `manual=1`, le premier titre de la file manuelle
// est marqué comme ajouté par une invitée. Deux invités (dont un exclu) dans les contrôles de
// l'hôte (#288).
const fakeQr = (() => {
  let cells = "";
  for (let y = 0; y < 21; y++)
    for (let x = 0; x < 21; x++) if ((x * 7 + y * 13 + x * y) % 3 === 0) cells += `M${x + 2} ${y + 2}h1v1h-1z`;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 25 25" shape-rendering="crispEdges"><rect width="25" height="25" fill="#ffffff"/><path fill="#000000" d="${cells}"/></svg>`;
})();
function mockJam(mode: "online" | "local", reconnecting = false) {
  const first = manual[0];
  return {
    mode,
    url:
      mode === "online"
        ? "https://jam.bebanana.ch/j/3f9c2a7e5d1b4c08a6e2f1d9b7c3a5e0/"
        : "http://192.168.1.20:47800/",
    code: "K7QX2M",
    lan: true,
    qrSvg: fakeQr,
    connection: reconnecting ? ("reconnecting" as const) : ("online" as const),
    reason: reconnecting ? "relais injoignable" : null,
    guests: first ? { [first.id]: "Léa" } : {},
    rules: {
      guestControls: true,
      maxPending: null as number | null,
      albums: true,
      guestVolume: false,
      guestVolumeMax: 80,
    },
    addsOpen: true,
    members: [
      { id: 1, name: "Léa" as string | null, added: 4, banned: false },
      { id: 2, name: null as string | null, added: 31, banned: true },
    ],
  };
}
let notificationTaken = false;
const jamParam = params.get("jam");
let jam: ReturnType<typeof mockJam> | null =
  jamParam === "1" || jamParam === "reconnecting"
    ? mockJam("online", jamParam === "reconnecting")
    : jamParam === "local"
      ? mockJam("local")
      : null;
/** Télécommande (#336) : `?remote=1` l'affiche allumée ; `?handoff=1` accepte les
 *  transferts (#414). Téléphone : `?platform=android&linked=1` a un ordinateur lié. */
let remoteCode = "H4KX9QMT2WZP";
let remoteOn = params.get("remote") === "1";
let handoffOn = params.get("handoff") === "1";
const mockRemote = () => ({
  remote: remoteOn,
  handoff: handoffOn,
  url: "http://192.168.1.20:47801/",
  code: remoteCode,
  lan: true,
  qrSvg: fakeQr,
});
let remote: ReturnType<typeof mockRemote> | null = remoteOn || handoffOn ? mockRemote() : null;
const remoteUpdate = () => (remote = remoteOn || handoffOn ? mockRemote() : null);
let linkedComputer: { host: string; syncedAt: number | null; online: boolean } | null =
  params.get("linked") === "1"
    ? { host: "192.168.1.20:47801", syncedAt: Date.now() - 3_600_000, online: true }
    : null;

const handlers: Record<string, (args: Record<string, unknown>) => unknown> = {
  ...blind.handlers,
  auth_status: () => {
    // Session non reprise (réseau absent au lancement, #252) : écran de connexion qui réessaie.
    if (params.has("authfail")) throw new Error("réseau : error sending request");
    return params.has("loggedout") ? null : { userId: 1, countryCode: "CH" };
  },
  login: () => ({ userId: 1, countryCode: "CH" }),
  // `?browserLogin=1` : connexion par le navigateur du système, comme sous Linux.
  login_in_browser: () => params.get("browserLogin") === "1",
  browser_login_start: () => "https://login.tidal.com/authorize?response_type=code&client_id=demo",
  browser_login_finish: () => ({ userId: 1, countryCode: "CH" }),
  logout: () => null,
  // Comme le backend (#207) : première page d'abord (la moitié des lignes), puis tout.
  home: () => ({
    sections: arrangeHome(withPlayed(fx.home.slice(0, homeFirstPage)), ui.home),
    more: homeFirstPage < fx.home.length,
  }),
  home_rest: () => arrangeHome(withPlayed(fx.home), ui.home),
  feed: () => {
    const items = fx.feed ?? [];
    emit("feed-unseen", items.filter((i) => !i.seen).length);
    return items;
  },
  mark_feed_seen: () => {
    fx.feed = (fx.feed ?? []).map((i) => ({ ...i, seen: true }));
    emit("feed-unseen", 0);
  },
  lyrics: ({ trackId }) => mockLyrics(Number(trackId)),
  track_info: ({ trackId }) => mockTrackInfo(Number(trackId)),
  album_credits: () => mockAlbumCredits(),
  // Approximation du parseur Rust : un titre ouvre toujours l'album de démonstration.
  open_link: ({ text }) => {
    // #328 : une playlist d'un autre service ouvre l'import.
    if (/(deezer\.com|open\.spotify\.com|music\.apple\.com)\/.*playlist/i.test(String(text))) {
      return { kind: "import", link: String(text).trim() };
    }
    if (/youtube\.com\/playlist/i.test(String(text))) {
      throw "Les playlists YouTube Music ne peuvent pas être importées : YouTube ne les montre pas sans compte";
    }
    const m = /tidal\.com\/(?:browse\/)?(track|album|artist|playlist|mix)\/([\w-]+)/i.exec(String(text));
    if (!m) return null;
    const [, kind, id] = m;
    if (kind === "track") return { kind: "album", id: fx.album.album.id, track: Number(id) };
    if (kind === "playlist" || kind === "mix") return kind === "playlist" ? { kind, uuid: id } : { kind, id };
    return { kind, id: Number(id) };
  },
  // #328 : progression simulée, puis des titres trouvés, à vérifier et introuvables.
  import_playlist: async () => {
    const pool = [...fx.search.tracks, ...fx.album.tracks];
    const total = Math.min(pool.length, 12);
    for (let done = 0; done <= total; done += 3) {
      emit("import-progress", { done, total });
      await new Promise((r) => setTimeout(r, 250));
    }
    const items = pool.slice(0, total).map((track, i) => ({
      title: track.title + (i % 5 === 3 ? " - 2011 Remaster" : ""),
      artist: track.artists[0]?.name ?? null,
      track: i % 6 === 5 ? null : track,
      sure: i % 5 !== 3,
    }));
    return { name: "Playlist d'un ami", service: "spotify", total: 150, read: 100, items };
  },
  cancel_import: () => undefined,
  search: ({ query }) => {
    const q = String(query).trim().replace(/\s+/g, " ");
    if (!q) return { tracks: [], albums: [], artists: [], playlists: [] };
    return fx.search;
  },
  search_history: () => searchHistory,
  note_search_history: ({ query }) => {
    const q = String(query).trim().replace(/\s+/g, " ");
    if (!q) return searchHistory;
    // Comme `search_history.rs` : en tête, sans doublon (casse ignorée), 20 au plus.
    searchHistory = [q, ...searchHistory.filter((h) => h.toLowerCase() !== q.toLowerCase())].slice(0, 20);
    return searchHistory;
  },
  remove_search_history: ({ query }) => {
    const q = String(query).trim().replace(/\s+/g, " ").toLowerCase();
    searchHistory = searchHistory.filter((h) => h.toLowerCase() !== q);
    return searchHistory;
  },
  clear_search_history: () => (searchHistory = []),
  play_history: () => ({ recent: playedAlbums, top: playedAlbums.slice(0, 2) }),
  clear_play_history: () => void (playedAlbums = []),
  play_stats: ({ period }) => {
    if (playedAlbums.length === 0) return { plays: 0, seconds: 0, unknown: 0, tracks: [], albums: [], artists: [] };
    const scale = ({ week: 1, month: 4, year: 30, all: 40 } as Record<string, number>)[String(period)] ?? 1;
    const tracks = fx.search.tracks.slice(0, 12).map((item, i) => ({ item, plays: (12 - i) * scale }));
    const artists = fx.search.artists.slice(0, 6).map((a, i) => ({ item: a, plays: (9 - i) * scale }));
    return {
      plays: tracks.reduce((n, t) => n + t.plays, 0),
      seconds: tracks.reduce((n, t) => n + t.plays * t.item.duration, 0),
      unknown: period === "all" ? 37 : 0,
      tracks,
      albums: playedAlbums.map((item, i) => ({ item, plays: (8 - i) * scale })),
      artists,
    };
  },
  // Rétrospective (#417) : les deux derniers mois et leur année, chiffres tirés des fixtures.
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
    const tracks = fx.search.tracks.slice(0, 10).map((item, i) => ({ item, plays: (14 - i) * scale }));
    const artists = fx.search.artists.slice(0, 6).map((item, i) => ({ item, plays: (11 - i) * scale }));
    const hours = Array.from({ length: 24 }, (_, h) => Math.round((h >= 8 ? 3 : 0) + (h >= 18 ? 6 : 0) + (h === 22 ? 4 : 0)) * scale);
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
      weekdays: [5, 4, 6, 4, 9, 12, 8].map((n) => n * scale),
      hours,
      topWeekday: 5,
      topHour: 22,
    };
  },
  // Playlists intelligentes (#383) : vides tant que l'historique l'est, comme le vrai backend.
  smart_list: ({ list }) => {
    if (playedAlbums.length === 0) return [];
    const offset = ({ topMonth: 0, onRepeat: 2, forgotten: 4, neverPlayed: 6 } as Record<string, number>)[String(list)] ?? 0;
    return fx.search.tracks.slice(offset, offset + 8);
  },
  jam_status: () => jam,
  jam_online_available: () => true,
  start_jam: (args) => (jam ??= mockJam(args.mode === "local" ? "local" : "online")),
  stop_jam: () => void (jam = null),
  remote_status: () => remote,
  start_remote: () => ((remoteOn = true), remoteUpdate()),
  stop_remote: () => ((remoteOn = false), remoteUpdate()),
  set_handoff_accepted: ({ enabled }) => ((handoffOn = !!enabled), remoteUpdate()),
  linked_computer: () => linkedComputer,
  link_computer: ({ link }) => {
    const m = /^\s*(?:http:\/\/)?([^/#\s]+)\/?[#\s]/.exec(String(link));
    if (!m) throw new Error("adresse non reconnue : copiez-la depuis Réglages › Application › Télécommande sur l'ordinateur");
    return (linkedComputer = { host: m[1], syncedAt: null, online: true });
  },
  unlink_computer: () => void (linkedComputer = null),
  handoff_pull: () => undefined,
  handoff_push: () => undefined,
  sync_history: () => (linkedComputer ? (linkedComputer.syncedAt = Date.now()) : null),
  renew_remote_code: () => {
    remoteCode = "QZ7M3KD8XF2N";
    return remote ? (remote = mockRemote()) : null;
  },
  set_jam_rules: (args) =>
    jam ? (jam = { ...jam, rules: args.rules as NonNullable<typeof jam>["rules"] }) : null,
  set_jam_adds_open: (args) => (jam ? (jam = { ...jam, addsOpen: args.open === true }) : null),
  set_jam_guest_banned: (args) =>
    jam
      ? (jam = {
          ...jam,
          members: jam.members.map((m) => (m.id === args.id ? { ...m, banned: args.banned === true } : m)),
        })
      : null,
  // Avec `?hirestwin=1`, l'album affiché n'est pas hi-res et a un jumeau qui l'est.
  album: () => (hiresTwin ? { ...fx.album, album: { ...fx.album.album, hires: false } } : fx.album),
  hires_twin_album: () =>
    hiresTwin ? { ...fx.album.album, id: fx.album.album.id + 1, title: `${fx.album.album.title} (Hi-Res)`, hires: true } : null,
  artist: () => fx.artist,
  album_related: () => ({
    versions: fx.search.albums.slice(0, 4).map((a, i) => ({ ...a, id: a.id + 1000 + i, title: `${fx.album.album.title} (Deluxe ${i + 1})` })),
    byArtist: fx.artist.albums.filter((a) => a.id !== fx.album.album.id).slice(0, 10),
  }),
  artist_extra: () => ({
    bio: {
      source: "TiVo",
      paragraphs: [
        [
          { kind: "text", text: "Biographie de démonstration. Influencé par " },
          { kind: "artist", text: fx.artist.artist.name, id: fx.artist.artist.id },
          { kind: "text", text: ", il enregistre " },
          { kind: "album", text: fx.album.album.title, id: fx.album.album.id },
          { kind: "text", text: " après plusieurs années de tournée." },
        ],
        [{ kind: "text", text: "Second paragraphe, ".repeat(30) }],
        [{ kind: "text", text: "Troisième paragraphe." }],
      ],
    },
    similar: fx.search.artists.slice(0, 6),
    appearsOn: fx.search.albums.slice(0, 6),
  }),
  // Comme le vrai backend (#152) : `null` si rien n'a changé depuis `revision`, et une copie
  // gardée pour `cached_playlist` (effacée par les modifications faites dans l'app).
  playlist: ({ uuid, revision }) => {
    const page = currentPlaylist(String(uuid));
    cachedPlaylists.set(String(uuid), structuredClone(page));
    return page.revision === revision ? null : page;
  },
  cached_playlist: ({ uuid }) => cachedPlaylists.get(String(uuid)) ?? null,
  likes: () => likedIds(),
  // Les arguments sont des proxys Svelte : copiés en JSON avant d'être gardés.
  set_liked: ({ target, liked: on }) => setLiked(JSON.parse(JSON.stringify(target)), Boolean(on)),
  set_tracks_liked: ({ trackIds, liked: on }) => {
    for (const id of trackIds as number[]) setLiked({ kind: "track", id }, Boolean(on));
  },
  blocked: () => structuredClone(blocked),
  set_blocked: ({ target, blocked: on }) => setBlocked(JSON.parse(JSON.stringify(target)), Boolean(on)),
  later_ids: () => laterIds(),
  save_for_later: () => libraryList([...laterItems]),
  set_saved_for_later: ({ target, saved }) =>
    setSavedForLater(JSON.parse(JSON.stringify(target)), Boolean(saved)),
  own_playlists: () => own.map((p) => ownPage(p.uuid)?.playlist ?? p),
  add_to_playlist: ({ uuid, trackIds }) => {
    cachedPlaylists.delete(String(uuid));
    return addToOwn(String(uuid), [...(trackIds as number[])]);
  },
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
    ownPages.set(uuid, { playlist: summary, tracks: [], revision: null });
    addToOwn(uuid, [...(trackIds as number[])]);
    return uuid;
  },
  playlist_cleanup: ({ uuid }) => cleanupReport(ownPage(String(uuid))?.tracks ?? []),
  clean_playlist: ({ ops }) => cleanupResult(ops as ListOp[], false),
  undo_playlist_cleanup: ({ ops }) => cleanupResult(ops as ListOp[], true),
  liked_cleanup: () => cleanupReport(fx.library.tracks),
  clean_liked: ({ ops }) =>
    cleanupResult(
      ops as ListOp[],
      (ops as ListOp[]).some((o) => o.op === "insert"),
    ),
  remove_from_playlist: ({ uuid, index, trackId }) => {
    cachedPlaylists.delete(String(uuid));
    const page = ownPage(String(uuid));
    if (page?.tracks[Number(index)]?.id !== Number(trackId)) {
      throw new Error("la playlist a changé, rouvrez-la avant de retirer ce titre");
    }
    removedFromPlaylist = page.tracks.splice(Number(index), 1)[0] ?? null;
    page.playlist.numberOfTracks = page.tracks.length;
  },
  restore_to_playlist: ({ uuid, index, trackId }) => {
    cachedPlaylists.delete(String(uuid));
    const page = ownPage(String(uuid));
    const entry = removedFromPlaylist;
    if (!page || !entry || entry.id !== Number(trackId)) return false;
    page.tracks.splice(Number(index), 0, entry);
    page.playlist.numberOfTracks = page.tracks.length;
    removedFromPlaylist = null;
    return true;
  },
  // Sélection multiple (#332) : positions retirées gardées pour l'annulation.
  remove_many_from_playlist: ({ uuid, items }) => {
    cachedPlaylists.delete(String(uuid));
    const page = ownPage(String(uuid));
    const list = [...(items as { index: number; trackId: number }[])].sort((a, b) => b.index - a.index);
    if (!page || list.some((i) => page.tracks[i.index]?.id !== i.trackId)) {
      throw new Error("la playlist a changé, rouvrez-la avant de la modifier");
    }
    removedMany = list.map((i) => page.tracks.splice(i.index, 1)[0]);
    page.playlist.numberOfTracks = page.tracks.length;
  },
  restore_many_to_playlist: ({ uuid, items }) => {
    cachedPlaylists.delete(String(uuid));
    const page = ownPage(String(uuid));
    if (!page) return 0;
    const list = [...(items as { index: number; trackId: number }[])].sort((a, b) => a.index - b.index);
    let restored = 0;
    for (const item of list) {
      const entry = removedMany.find((e) => e.id === item.trackId);
      if (!entry) continue;
      page.tracks.splice(item.index, 0, entry);
      restored++;
    }
    removedMany = [];
    page.playlist.numberOfTracks = page.tracks.length;
    return restored;
  },
  move_in_playlist: ({ uuid, from, to, trackId }) => {
    cachedPlaylists.delete(String(uuid));
    const page = ownPage(String(uuid));
    if (page?.tracks[Number(from)]?.id !== Number(trackId)) {
      throw new Error("la playlist a changé, rouvrez-la avant de la modifier");
    }
    const [moved] = page.tracks.splice(Number(from), 1);
    page.tracks.splice(Number(to), 0, moved);
  },
  edit_playlist: ({ uuid, title, description }) => {
    cachedPlaylists.delete(String(uuid));
    const page = ownPage(String(uuid));
    if (!page) throw new Error("playlist introuvable");
    page.playlist.title = String(title);
    page.playlist.description = String(description) || null;
    for (const list of [own, libPlaylists]) {
      const p = list.find((x) => x.uuid === uuid);
      if (p) Object.assign(p, { title: page.playlist.title, description: page.playlist.description });
    }
  },
  playlist_folders: () =>
    [...mockFolders]
      .sort((a, b) => a.name.localeCompare(b.name))
      .map((f) => ({ ...f, playlists: [...f.playlists] })),
  create_folder: ({ name, uuids }) => {
    const list = [...(uuids as string[])];
    unfile(list);
    const folder: PlaylistFolder = { id: crypto.randomUUID(), name: String(name).trim(), playlists: list };
    mockFolders = [...mockFolders, folder];
    return { ...folder, playlists: [...list] };
  },
  rename_folder: ({ id, name }) => {
    mockFolders = mockFolders.map((f) => (f.id === id ? { ...f, name: String(name).trim() } : f));
  },
  move_to_folder: ({ uuids, folderId }) => {
    const list = [...(uuids as string[])];
    unfile(list);
    mockFolders = mockFolders.map((f) => (f.id === folderId ? { ...f, playlists: [...f.playlists, ...list] } : f));
  },
  delete_folder: ({ id }) => {
    mockFolders = mockFolders.filter((f) => f.id !== id);
  },
  delete_playlist: ({ uuid }) => {
    cachedPlaylists.delete(String(uuid));
    ownPages.delete(String(uuid));
    for (const list of [own, libPlaylists]) {
      const i = list.findIndex((p) => p.uuid === uuid);
      if (i >= 0) list.splice(i, 1);
    }
  },
  mix: () => mixPage,
  // La radio factice garde ses titres, mais reprend le point de départ demandé.
  radio: ({ seed, id }) => ({ ...radioPage, seed, id }),
  library_mixes: () => libraryMixes,
  library_tracks: () => libraryList(longCount ? longTracks : fx.library.tracks),
  library_albums: () => libraryList([...favAlbums]),
  library_artists: () => libraryList([...favArtists]),
  library_playlists: () => libraryList([...libPlaylists]),
  // Copie « du lancement précédent » (#181) : la liste actuelle, comme si rien n'avait changé.
  cached_library: ({ tab }) =>
    (<Record<string, () => unknown>>{
      tracks: () => libraryList(longCount ? longTracks : fx.library.tracks),
      albums: () => libraryList([...favAlbums]),
      artists: () => libraryList([...favArtists]),
      playlists: () => libraryList([...libPlaylists]),
    })[String(tab)]?.() ?? null,
  player_state: () => state,
  queue_state: () => queueView(),
  queue_track_ids: () => {
    const v = queueView();
    return [...v.manual, ...v.upcoming].map((t) => t.id);
  },
  ui_settings: () => ui,
  // `?update=1` : simule une mise à jour prête.
  update_status: () => (mockDownloaded ? mockUpdate() : null),
  install_update: () => null,
  // `?updates=manual` : Linux hors d'une AppImage, pas de mise à jour automatique (#314).
  updates_available: () => params.get("updates") !== "manual",
  // Sans `?update=1`, l'app est « à jour ». `?update=download` : comme le bureau (#187),
  // la réponse arrive avant l'installeur, `update-ready` 2 s plus tard.
  check_for_updates: () => {
    const u = mockUpdate();
    if (u && !mockDownloaded) {
      setTimeout(() => {
        mockDownloaded = true;
        emit("update-ready", u);
      }, 2000);
      return { ...u, downloading: true };
    }
    if (u) emit("update-ready", u);
    return u;
  },
  set_ui_settings: ({ settings }) => {
    // Proxy Svelte : copie en JSON (voir les épingles dans docs/interface.md).
    ui = JSON.parse(JSON.stringify(settings)) as UiSettings;
  },
  language: () => effectiveLanguage(ui.language),
  set_language: ({ language }) => {
    ui.language = language as LanguagePref;
    return effectiveLanguage(ui.language);
  },
  set_home_layout: ({ sections }) => {
    const list = sections as { key: string; hidden: boolean }[];
    ui.home = {
      ...ui.home,
      order: list.map((s) => s.key),
      hidden: list.filter((s) => s.hidden).map((s) => s.key),
    };
    return ui.home;
  },
  set_home_featured: ({ shown }) => (ui.home = { ...ui.home, featuredHidden: !shown }),
  open_project_page: ({ page }) => {
    console.info("open_project_page", page);
    return null;
  },
  // #192 : `?crash=1` simule un plantage au lancement précédent (toast et rapport).
  take_crash_notice: () => params.get("crash") === "1",
  last_crash_report: () =>
    params.get("crash") === "1" ? "# Rapport de plantage Seaglass\n\npanic dans le thread « main » : exemple\n" : null,
  changelog: () => [
    { version: "0.5.12", published: null, entries: ["Journal des versions dans Réglages › Application (#340)."] },
    { version: "0.5.11", published: "2026-11-11", entries: ["Taille de l'interface réglable avec `Ctrl` + « + » (#337)."] },
  ],
  current_log: () => "2026-11-11T10:00:00Z  INFO seaglass_lib: Seaglass (aperçu)\n",
  open_log_dir: () => null,
  // #330 : progression simulée sur trois playlists.
  export_library: async () => {
    for (let done = 0; done <= 3; done++) {
      emit("library-export", { done, total: 3 });
      await new Promise((r) => setTimeout(r, 400));
    }
    return {
      dir: "C:\\Users\\aperçu\\Documents\\Seaglass\\export-2026-11-12-182600",
      tracks: 120,
      albums: 14,
      artists: 9,
      playlists: 3,
      failed: [],
      truncated: false,
    };
  },
  cancel_library_export: () => null,
  export_settings: () => JSON.stringify({ format: "seaglass-settings", version: 1, ui }, null, 2),
  import_settings: () => ({ applied: ["ui.theme"], ignored: ["playback.wifiQuality"] }),
  reset_home_layout: () => (ui.home = { order: [], hidden: [], featuredHidden: false }),
  // Copie JSON : l'UI envoie des proxys Svelte, que `structuredClone` refuse.
  set_pins: ({ pins }) => (ui.pins = JSON.parse(JSON.stringify(pins))),
  // Comme le backend : le plus récent en tête, le tri par défaut de la liste est oublié.
  set_list_sort: ({ list, sort }) => {
    const id = list as string;
    const next = sort as TrackSort | null;
    const fallback = id === "likedTracks" ? "added:true" : "position:false";
    ui.sorts = (ui.sorts ?? []).filter((s) => s.list !== id);
    if (next && `${next.key}:${next.descending}` !== fallback) {
      ui.sorts.unshift({ list: id, key: next.key, descending: next.descending });
    }
    return ui.sorts;
  },
  playback_settings: () => playback,
  // Même validation que le backend : seules les tailles proposées sont acceptées.
  set_playback_settings: ({ settings }) => {
    const next = settings as PlaybackSettings;
    if (![32, 64, 128, 256].includes(next.cacheSize)) {
      throw new Error(`taille de cache non prise en charge : ${next.cacheSize} Mio`);
    }
    playback = JSON.parse(JSON.stringify(next));
    state.outputDevice = outputDevice();
    state.normalization = mockGain();
    applyExclusive();
    emit("player-state", state);
  },
  audio_devices: () => audioDevices,
  // `?platform=android` : mise en page tactile.
  platform: () => (params.get("platform") === "android" ? "android" : "desktop"),
  autostart_enabled: () => autostart,
  discord_available: () => true,
  exclusive_available: () => true,
  global_shortcuts_status: () => ({
    available: true,
    failed: ui.globalShortcuts.enabled && params.get("hotkeys") === "taken" ? ["playPause"] : [],
  }),
  // `?notification=1` : appui sur la notification d'Android au lancement (#163, #240), une fois.
  take_now_playing_request: () => {
    const asked = params.get("notification") === "1" && !notificationTaken;
    notificationTaken = true;
    return asked;
  },
  take_shared_link: () => null,
  // Mode conduite (#193) : `?platform=android&drive=1` l'ouvre au lancement (`initialView`).
  take_drive_request: () => false,
  // Raccourci d'une épingle (#335) : `?platform=android&pinshortcut=1` lit la première épingle.
  take_pin_request: () => {
    const pin = pinShortcutParam ? (ui.pins[0] ?? null) : null;
    pinShortcutParam = false;
    return pin;
  },
  keep_screen_on: () => null,

  // `?platform=android&links=1` : liens TIDAL déjà donnés à l'app.
  link_handling: () => (params.get("platform") === "android" ? { enabled: params.get("links") === "1" } : null),
  open_link_settings: () => undefined,
  // Le navigateur garde sa taille : seul l'état change (le bouton suit).
  fullscreen: () => fullscreen,
  set_fullscreen: ({ fullscreen: on }) => {
    fullscreen = Boolean(on);
    emit("fullscreen", fullscreen);
    return fullscreen;
  },
  // Hors Tauri, une seule page : le mini-lecteur se prévisualise avec `?mini`.
  open_mini_player: () => undefined,
  close_mini_player: () => undefined,
  mini_player_on_top: () => miniOnTop,
  set_mini_player_on_top: ({ onTop }) => {
    miniOnTop = Boolean(onTop);
    return miniOnTop;
  },
  set_autostart: ({ enabled }) => {
    autostart = Boolean(enabled);
    return autostart;
  },
  play_tracks: ({ trackIds, startIndex, shuffle, source }) => {
    context = (trackIds as number[]).map((id) => allTracks.get(id)).filter((t): t is Track => !!t);
    radioSeed = null;
    playSource = source ? (JSON.parse(JSON.stringify(source)) as PlaySource) : null;
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
    queueUndo = { manual: [...manual], context, index, radioSeed, playSource };
    manual.splice(Number(i), 1);
    emit("queue-state", queueView());
  },
  move_in_queue: ({ from, to }) => {
    const [f, t] = [Number(from), Number(to)];
    if (f === t || f >= manual.length || t >= manual.length) return;
    manual.splice(t, 0, ...manual.splice(f, 1));
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
        : { kind: req.kind };
    emit("player-state", state);
  },
  restore_queue: () => {
    if (!queueUndo) return false;
    manual.splice(0, manual.length, ...queueUndo.manual);
    ({ context, index, radioSeed, playSource } = queueUndo);
    queueUndo = null;
    state.hasNext = true;
    emit("player-state", state);
    emit("queue-state", queueView());
    return true;
  },
  clear_queue: () => {
    queueUndo = { manual: [...manual], context, index, radioSeed, playSource };
    manual.length = 0;
    radioSeed = null;
    playSource = null;
    context = context.slice(index, index + 1);
    index = 0;
    state.hasNext = false;
    emit("player-state", state);
    emit("queue-state", queueView());
  },
  play_from_queue: ({ part, index: i, trackId }) => {
    const at = Number(i);
    if (part === "manual") {
      if (manual[at]?.id !== Number(trackId)) return;
      manual.splice(0, at);
      return setCurrent(manual.shift());
    }
    if (context[index + 1 + at]?.id !== Number(trackId)) return;
    index += 1 + at;
    setCurrent(context[index]);
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
  // `home_rest` lit une page de plus (~0,5 s pour de vrai).
  const delay =
    cmd === "playlist" ? slowPlaylist : cmd === "home_rest" || cmd.endsWith("_cleanup") ? 500 : 0;
  await new Promise((r) => setTimeout(r, 120 + delay));
  // Un gestionnaire peut être asynchrone (progression émise en route, #330).
  return structuredClone(await handler(args)) as T;
}

export async function listen<T>(event: string, cb: (e: { payload: T }) => void) {
  const set = listeners.get(event) ?? new Set();
  set.add(cb as Listener);
  listeners.set(event, set);
  // Lecture du fil en fond au lancement (#415), comme `commands::feed::spawn_watch`.
  if (event === "feed-unseen") {
    setTimeout(() => emit("feed-unseen", (fx.feed ?? []).filter((i) => !i.seen).length), 500);
  }
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
            track: { ...track, id: track.id + 1_000_000, hires: true, album: { ...track.album, title: hiresEdition.title } },
            stream: hires,
            played: playback.hiresUpgrade === "auto",
          }
        : null,
    credits: [
      { role: "Composer", contributors: track.artists.map((a) => ({ name: a.name, artistId: a.id })) },
      { role: "Lyricist", contributors: [{ name: "Ann Demo", artistId: null }] },
      {
        role: "Electric Guitar",
        contributors: [
          { name: track.artists[0]?.name ?? "Demo", artistId: track.artists[0]?.id ?? null },
          { name: "Bob Session", artistId: null },
        ],
      },
      { role: "Producer", contributors: [{ name: "Carla Studio", artistId: null }] },
      { role: "Mastering Engineer", contributors: [{ name: "Dan Lathe", artistId: null }] },
      { role: "Theremin Wrangler", contributors: [{ name: "Eve Unknown", artistId: null }] },
    ],
  };
}

function mockAlbumCredits(): AlbumCreditGroup[] {
  const ids = fx.album.tracks.map((t) => t.id);
  const main = fx.album.album.artists[0];
  return [
    { role: "Composer", contributors: [{ name: main?.name ?? "Demo", artistId: main?.id ?? null, tracks: [] }] },
    {
      role: "Drums",
      contributors: [
        { name: "Dee Kit", artistId: null, tracks: ids.filter((_, i) => i % 2 === 0) },
        { name: "Ed Brush", artistId: null, tracks: ids.slice(1, 4) },
      ],
    },
    { role: "Producer", contributors: [{ name: "Carla Studio", artistId: null, tracks: [] }] },
    { role: "Mastering Engineer", contributors: [{ name: "Dan Lathe", artistId: null, tracks: ids.slice(-1) }] },
  ];
}

/** Paramètres d'URL de navigation, appliqués une fois l'app montée. */
export function initialView(): {
  route: Route | null;
  nowPlaying: boolean;
  queue: boolean;
  lyrics: boolean;
  trackInfo: boolean;
  drive: boolean;
  palette: boolean;
} {
  const routes: Record<string, Route> = {
    album: { name: "album", id: fx.album.album.id },
    artist: { name: "artist", id: fx.artist.artist.id },
    playlist: { name: "playlist", uuid: fx.playlist.playlist.uuid },
    mix: { name: "mix", id: mixPage.mix.id },
    radio: { name: "radio", seed: radioPage.seed, id: radioPage.id },
    library: { name: "library", tab: (params.get("tab") as LibraryTab) ?? "playlists" },
    stats: { name: "stats" },
    recap: { name: "recap" },
    folder: { name: "folder", id: "f0000000-0000-4000-8000-000000000001" },
    smart: { name: "smart", list: (params.get("list") as SmartList) ?? "topMonth" },
    import: { name: "import", link: "https://open.spotify.com/playlist/37i9dQZF1DX4UtSsGT1Sbe" },
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
    drive: params.has("drive"),
    palette: params.has("palette"),
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
    ai: false,
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
    ai: false,
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
      revision: null,
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
