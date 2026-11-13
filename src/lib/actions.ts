// Registre unique des actions de l'app (#412) : la palette de commandes les affiche avec
// leur raccourci, et les raccourcis clavier de `App.svelte` les exécutent par `runAction`.
// Une action ajoutée ici apparaît dans la palette ; son raccourci, s'il y en a un, se
// déclare dans `shortcut` (texte affiché) et se reconnaît dans `App.svelte`.

import { commands, type Repeat, type Theme } from "./ipc";
import { t } from "./i18n.svelte";
import { platform } from "./platform.svelte";
import { THEME_INFO } from "./themes";
import { player, stepVolume } from "./stores/player.svelte";
import { isLiked, toggleLike } from "./stores/likes.svelte";
import { back, canGoBack, current, go, openSettings, replace, type LibraryTab, type Route, type SettingsSection } from "./stores/nav.svelte";
import { setTheme, stepZoom, ui } from "./stores/ui.svelte";
import { toggleFullscreen } from "./stores/window.svelte";
import { pushToast } from "./stores/toasts.svelte";
import { playbackSettingsChanged } from "./stores/palette.svelte";

export type ActionGroup = "go" | "playback" | "window" | "theme";

export interface Action {
  id: string;
  group: ActionGroup;
  /** Libellé dans la langue courante (lu à l'ouverture de la palette). */
  label: () => string;
  keywords?: string[];
  /** Raccourci affiché dans la palette (reconnu par `App.svelte`) ; fonction s'il se traduit. */
  shortcut?: string | (() => string);
  /** Proposée seulement si vrai (connecté, bureau, titre en cours…). */
  available?: () => boolean;
  run: () => unknown;
}

/** Ce que seul `App.svelte` sait faire : ses panneaux et l'état de la session. */
export interface ActionContext {
  loggedIn: () => boolean;
  openNowPlaying: () => void;
  closeNowPlaying: () => void;
  toggleQueue: () => void;
}

let context: ActionContext = {
  loggedIn: () => false,
  openNowPlaying: () => {},
  closeNowPlaying: () => {},
  toggleQueue: () => {},
};

export function setActionContext(ctx: ActionContext) {
  context = ctx;
}

/** Mode exclusif possible sur cette machine (lu une fois, bureau seulement). */
let exclusiveAvailable = false;

export async function initActions() {
  if (platform.mobile) return;
  exclusiveAvailable = await commands.exclusiveAvailable().catch(() => false);
}

const desktop = () => !platform.mobile;
const session = () => context.loggedIn();
const hasTrack = () => session() && !!player.currentTrack;

/** Ouvre une page en fermant « En lecture » ; la même page n'est pas empilée deux fois. */
export function open(route: Route) {
  context.closeNowPlaying();
  if (JSON.stringify(route) === JSON.stringify(current())) return;
  go(route);
}

function goAction(id: string, label: () => string, route: Route, keywords?: string[]): Action {
  return { id, group: "go", label, keywords, available: session, run: () => open(route) };
}

const LIBRARY_TABS: LibraryTab[] = ["mixes", "albums", "artists", "tracks", "later"];
const SETTINGS_SECTIONS: SettingsSection[] = ["playback", "app"];
const REPEAT_NEXT: Record<Repeat, Repeat> = { off: "all", all: "one", one: "off" };
const SLEEP_MINUTES = [15, 30, 60];

async function toggleExclusive() {
  const settings = await commands.playbackSettings();
  const exclusive = !settings.exclusive;
  await commands.setPlaybackSettings({ ...settings, exclusive });
  playbackSettingsChanged.count++;
  pushToast({ level: "info", message: exclusive ? t.palette.actions.exclusiveOn : t.palette.actions.exclusiveOff });
}

function themeName(id: Theme): string {
  return t.settings.themes[id].name;
}

const a = () => t.palette.actions;

export const ACTIONS: Action[] = [
  // Pages
  goAction("go.home", () => a().home, { name: "home" }, ["home"]),
  goAction("go.feed", () => a().feed, { name: "feed" }, ["feed", "news"]),
  {
    id: "go.search",
    group: "go",
    label: () => a().search,
    keywords: ["search"],
    shortcut: "/",
    available: session,
    run: () => {
      context.closeNowPlaying();
      if (current().name !== "search") replace({ name: "search" });
      requestAnimationFrame(() => document.querySelector<HTMLInputElement>('input[type="search"]')?.focus());
    },
  },
  goAction("go.library", () => a().library, { name: "library", tab: "playlists" }, ["library", "playlists"]),
  ...LIBRARY_TABS.map((tab) =>
    goAction(`go.library.${tab}`, () => a().libraryTab(t.library.tabs[tab]), { name: "library", tab }, ["library"]),
  ),
  goAction("go.stats", () => a().stats, { name: "stats" }, ["stats"]),
  goAction("go.recap", () => a().recap, { name: "recap" }, ["recap", "wrapped", "rétrospective", "bilan"]),
  {
    id: "go.settings",
    group: "go",
    label: () => a().settings,
    keywords: ["settings", "options", "preferences"],
    shortcut: "Ctrl+,",
    run: () => {
      context.closeNowPlaying();
      openSettings();
    },
  },
  ...SETTINGS_SECTIONS.map(
    (section): Action => ({
      id: `go.settings.${section}`,
      group: "go",
      label: () => a().settingsSection(t.settings.sections[section]),
      keywords: ["settings"],
      run: () => {
        context.closeNowPlaying();
        openSettings(section);
      },
    }),
  ),
  {
    id: "go.nowPlaying",
    group: "go",
    label: () => a().nowPlaying,
    keywords: ["now", "playing", "lyrics", "paroles"],
    available: hasTrack,
    run: () => context.openNowPlaying(),
  },
  {
    id: "go.queue",
    group: "go",
    label: () => a().queue,
    keywords: ["queue", "file"],
    available: () => session() && desktop(),
    run: () => context.toggleQueue(),
  },
  {
    id: "go.back",
    group: "go",
    label: () => a().back,
    keywords: ["back", "retour"],
    shortcut: "Alt+←",
    available: canGoBack,
    run: back,
  },

  // Lecture
  {
    id: "play.toggle",
    group: "playback",
    label: () => a().togglePause,
    keywords: ["play", "pause"],
    shortcut: () => t.palette.space,
    available: hasTrack,
    run: () => commands.togglePause(),
  },
  { id: "play.next", group: "playback", label: () => a().next, keywords: ["next", "skip"], available: () => session() && player.state.hasNext, run: () => commands.next() },
  { id: "play.previous", group: "playback", label: () => a().previous, keywords: ["previous"], available: hasTrack, run: () => commands.previous() },
  {
    id: "play.shuffle",
    group: "playback",
    label: () => (player.state.shuffle ? a().shuffleOff : a().shuffleOn),
    keywords: ["shuffle", "melanger"],
    available: session,
    run: () => commands.setShuffle(!player.state.shuffle),
  },
  {
    id: "play.repeat",
    group: "playback",
    label: () => a().repeat(a().repeatModes[REPEAT_NEXT[player.state.repeat]]),
    keywords: ["repeat", "boucle", "loop"],
    available: session,
    run: () => commands.setRepeat(REPEAT_NEXT[player.state.repeat]),
  },
  {
    id: "play.like",
    group: "playback",
    label: () => (player.currentTrack && isLiked(player.currentTrack.id) ? a().unlike : a().like),
    keywords: ["like", "favori", "coeur", "heart"],
    available: hasTrack,
    run: () => player.currentTrack && toggleLike(player.currentTrack),
  },
  { id: "play.volumeUp", group: "playback", label: () => a().volumeUp, keywords: ["volume"], shortcut: "Ctrl+↑", available: session, run: () => stepVolume(1) },
  { id: "play.volumeDown", group: "playback", label: () => a().volumeDown, keywords: ["volume"], shortcut: "Ctrl+↓", available: session, run: () => stepVolume(-1) },
  {
    id: "play.mute",
    group: "playback",
    label: () => (player.state.volume.muted ? a().unmute : a().mute),
    keywords: ["mute", "volume", "son"],
    available: session,
    run: () => commands.setMuted(!player.state.volume.muted),
  },
  {
    id: "play.exclusive",
    group: "playback",
    label: () => a().exclusive,
    keywords: ["exclusive", "wasapi", "bit-perfect"],
    available: () => session() && desktop() && exclusiveAvailable,
    run: toggleExclusive,
  },
  ...SLEEP_MINUTES.map(
    (minutes): Action => ({
      id: `sleep.${minutes}`,
      group: "playback",
      label: () => a().sleepMinutes(minutes),
      keywords: ["sleep", "timer"],
      available: session,
      run: () => commands.setSleepTimer({ kind: "minutes", minutes }),
    }),
  ),
  {
    id: "sleep.endOfTrack",
    group: "playback",
    label: () => a().sleepEndOfTrack,
    keywords: ["sleep", "timer"],
    available: hasTrack,
    run: () => commands.setSleepTimer({ kind: "endOfTrack" }),
  },
  {
    id: "sleep.cancel",
    group: "playback",
    label: () => a().sleepCancel,
    keywords: ["sleep", "timer"],
    available: () => session() && !!player.state.sleep,
    run: () => commands.setSleepTimer(null),
  },

  // Fenêtre
  {
    id: "window.mini",
    group: "window",
    label: () => a().miniPlayer,
    keywords: ["mini", "player"],
    shortcut: "Ctrl+M",
    available: () => session() && desktop(),
    run: () => commands.openMiniPlayer(),
  },
  { id: "window.fullscreen", group: "window", label: () => a().fullscreen, keywords: ["fullscreen"], shortcut: "F11", available: desktop, run: toggleFullscreen },
  { id: "window.zoomIn", group: "window", label: () => a().zoomIn, keywords: ["zoom", "taille"], shortcut: "Ctrl++", available: desktop, run: () => stepZoom(1) },
  { id: "window.zoomOut", group: "window", label: () => a().zoomOut, keywords: ["zoom", "taille"], shortcut: "Ctrl+-", available: desktop, run: () => stepZoom(-1) },
  { id: "window.zoomReset", group: "window", label: () => a().zoomReset, keywords: ["zoom", "taille"], shortcut: "Ctrl+0", available: desktop, run: () => stepZoom(0) },

  // Thèmes : un par entrée, sauf le thème en cours.
  ...THEME_INFO.map(
    (info): Action => ({
      id: `theme.${info.id}`,
      group: "theme",
      label: () => a().theme(themeName(info.id)),
      keywords: ["theme", info.id, info.scheme === "dark" ? "sombre dark" : "clair light"],
      available: () => ui.settings.theme !== info.id,
      run: () => setTheme(info.id),
    }),
  ),
];

const byId = new Map(ACTIONS.map((action) => [action.id, action]));

export function shortcutText(action: Action): string | null {
  const s = action.shortcut;
  return typeof s === "function" ? s() : (s ?? null);
}

/** Actions proposées maintenant, dans l'ordre du registre. */
export function availableActions(): Action[] {
  return ACTIONS.filter((action) => !action.available || action.available());
}

/** Exécute une action ; une erreur part dans un toast. Sans effet si elle n'est pas proposée. */
export function runAction(id: string) {
  const action = byId.get(id);
  if (!action || (action.available && !action.available())) return;
  try {
    const result = action.run();
    if (result instanceof Promise) result.catch((e) => pushToast({ level: "error", message: String(e) }));
  } catch (e) {
    pushToast({ level: "error", message: String(e) });
  }
}
