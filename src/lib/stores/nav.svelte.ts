// Navigation interne : une pile de routes, sans URL (l'app n'a qu'une fenêtre).

import type { RadioSeed, SmartList } from "../ipc";

export type LibraryTab = "playlists" | "mixes" | "albums" | "artists" | "tracks" | "later";

export type SettingsSection = "appearance" | "playback" | "app";

export type Route =
  | { name: "home" }
  | { name: "feed" }
  | { name: "search" }
  | { name: "library"; tab: LibraryTab }
  /** Statistiques d'écoute locales (#326), ouvertes depuis la bibliothèque. */
  | { name: "stats" }
  /** Rétrospective du mois et de l'année (#417), ouverte depuis les statistiques. */
  | { name: "recap" }
  /** Playlist intelligente locale (#383). */
  | { name: "smart"; list: SmartList }
  | { name: "settings"; section: SettingsSection }
  /** `track` : titre mis en évidence (lien collé, #203). */
  | { name: "album"; id: number; track?: number }
  | { name: "artist"; id: number }
  | { name: "playlist"; uuid: string }
  /** Dossier de playlists de la bibliothèque (#331). */
  | { name: "folder"; id: string }
  | { name: "mix"; id: string }
  /** Playlist d'un autre service à importer (#328). */
  | { name: "import"; link: string }
  | { name: "radio"; seed: RadioSeed; id: number };

const MAX_HISTORY = 50;

export const nav = $state<{ stack: Route[] }>({ stack: [{ name: "home" }] });

export function current(): Route {
  return nav.stack[nav.stack.length - 1];
}

export function go(route: Route) {
  nav.stack = [...nav.stack, route].slice(-MAX_HISTORY);
}

/** Remplace la page courante (changement d'onglet, entrée du menu). */
export function replace(route: Route) {
  nav.stack = [...nav.stack.slice(0, -1), route];
}

/** Ouvre une section des réglages : change de section si la page est déjà ouverte. */
export function openSettings(section: SettingsSection = "appearance") {
  const route: Route = { name: "settings", section };
  if (current().name === "settings") replace(route);
  else go(route);
}

export function back() {
  if (nav.stack.length > 1) nav.stack = nav.stack.slice(0, -1);
}

export function canGoBack(): boolean {
  return nav.stack.length > 1;
}
