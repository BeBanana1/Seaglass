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
// active), `?eq=1` (égaliseur activé avec un profil AutoEQ, #428), `?exclusive=1` (mode exclusif WASAPI, #245), `?slow=<ms>` (ouverture d'une playlist ralentie : « Chargement… » après 300 ms,
// copie gardée affichée aussitôt à la réouverture, #152).

import {
  emit,
  fx,
  listeners,
  mixPage,
  params,
  radioPage,
  type Handlers,
  type Listener,
} from "./core";
import { searchDemo } from "./demo";
import { detailsHandlers } from "./details";
import { favoritesHandlers } from "./favorites";
import { historyHandlers, playedAlbums } from "./history";
import { jamHandlers } from "./jam";
import { blind, playerHandlers } from "./player";
import { playlistsHandlers, slowPlaylist } from "./playlists";
import { effectiveLanguage, saveDemoUi, settingsHandlers, ui } from "./settings";
import type { HomeLayout, HomeRow, HomeSection, SmartList } from "../ipc";
import type { LibraryTab, Route, SettingsSection } from "../stores/nav.svelte";

export { playbackTick } from "./player";

/** `?hirestwin=1` : la page album propose un album jumeau hi-res (#59). */
const hiresTwin = params.has("hirestwin");

/** « Écoutés dans Seaglass » en tête du flux, comme le vrai backend. */
function withPlayed(rows: HomeRow[]): HomeRow[] {
  if (playedAlbums.length === 0) return rows;
  const items = playedAlbums.map((a) => ({ kind: "album" as const, ...a }));
  const title =
    effectiveLanguage(ui.language) === "en" ? "Played in Seaglass" : "Écoutés dans Seaglass";
  return [{ key: "SEAGLASS_RECENT", title, items }, ...rows];
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

const homeFirstPage = Math.ceil(fx.home.length / 2);

let searchHistory: string[] = __DEMO__
  ? ["Lumen Tide", "Warm Currents", "Night Ferry"]
  : ["Daft Punk", "Moon Safari", "Bohemian Rhapsody"];

const handlers: Handlers = {
  ...blind.handlers,
  ...playerHandlers,
  ...favoritesHandlers,
  ...playlistsHandlers,
  ...historyHandlers,
  ...jamHandlers,
  ...detailsHandlers,
  ...settingsHandlers,
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
  // Approximation du parseur Rust : un titre ouvre toujours l'album de démonstration.
  open_link: ({ text }) => {
    // #328 : une playlist d'un autre service ouvre l'import.
    if (/(deezer\.com|open\.spotify\.com|music\.apple\.com)\/.*playlist/i.test(String(text))) {
      return { kind: "import", link: String(text).trim() };
    }
    if (/youtube\.com\/playlist/i.test(String(text))) {
      throw "Les playlists YouTube Music ne peuvent pas être importées : YouTube ne les montre pas sans compte";
    }
    const m = /tidal\.com\/(?:browse\/)?(track|album|artist|playlist|mix)\/([\w-]+)/i.exec(
      String(text),
    );
    if (!m) return null;
    const [, kind, id] = m;
    if (kind === "track") return { kind: "album", id: fx.album.album.id, track: Number(id) };
    if (kind === "playlist" || kind === "mix")
      return kind === "playlist" ? { kind, uuid: id } : { kind, id };
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
    return fx.pages ? searchDemo(q) : fx.search;
  },
  search_history: () => searchHistory,
  note_search_history: ({ query }) => {
    const q = String(query).trim().replace(/\s+/g, " ");
    if (!q) return searchHistory;
    // Comme `search_history.rs` : en tête, sans doublon (casse ignorée), 20 au plus.
    searchHistory = [q, ...searchHistory.filter((h) => h.toLowerCase() !== q.toLowerCase())].slice(
      0,
      20,
    );
    return searchHistory;
  },
  remove_search_history: ({ query }) => {
    const q = String(query).trim().replace(/\s+/g, " ").toLowerCase();
    searchHistory = searchHistory.filter((h) => h.toLowerCase() !== q);
    return searchHistory;
  },
  clear_search_history: () => (searchHistory = []),
  // Avec `?hirestwin=1`, l'album affiché n'est pas hi-res et a un jumeau qui l'est.
  album: ({ id }) => {
    const page = fx.pages?.albums.find((p) => p.album.id === Number(id)) ?? fx.album;
    return hiresTwin ? { ...page, album: { ...page.album, hires: false } } : page;
  },
  hires_twin_album: () =>
    hiresTwin
      ? {
          ...fx.album.album,
          id: fx.album.album.id + 1,
          title: `${fx.album.album.title} (Hi-Res)`,
          hires: true,
        }
      : null,
  artist: ({ id }) => fx.pages?.artists.find((p) => p.artist.id === Number(id)) ?? fx.artist,
  album_related: () => ({
    versions: fx.search.albums.slice(0, 4).map((a, i) => ({
      ...a,
      id: a.id + 1000 + i,
      title: `${fx.album.album.title} (Deluxe ${i + 1})`,
    })),
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
  mix: ({ id }) => fx.pages?.mixes.find((p) => p.mix.id === String(id)) ?? mixPage,
  // La radio factice garde ses titres, mais reprend le point de départ demandé.
  radio: ({ seed, id }) => ({ ...radioPage, seed, id }),
};

export async function invoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const handler = handlers[cmd];
  if (!handler) throw new Error(`commande factice inconnue : ${cmd}`);
  // Petite latence pour voir les états de chargement.
  // `home_rest` lit une page de plus (~0,5 s pour de vrai).
  const delay =
    cmd === "playlist" ? slowPlaylist : cmd === "home_rest" || cmd.endsWith("_cleanup") ? 500 : 0;
  await new Promise((r) => setTimeout(r, 120 + delay));
  // A handler can be asynchronous (progress emitted along the way, #330).
  const result = structuredClone(await handler(args)) as T;
  if (__DEMO__) saveDemoUi();
  return result;
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
    contributor: { name: "contributor", id: fx.artist.artist.id },
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
