// Internal navigation: a stack of routes, with no URL (the app has a single window).

import type { RadioSeed, SmartList } from "../ipc";

export type LibraryTab = "playlists" | "mixes" | "albums" | "artists" | "tracks" | "later";

export type SettingsSection = "appearance" | "playback" | "app";

export type Route =
  | { name: "home" }
  | { name: "feed" }
  | { name: "search" }
  | { name: "library"; tab: LibraryTab }
  /** Local listening statistics (#326), opened from the library. */
  | { name: "stats" }
  /** Recap of the month and the year (#417), opened from the statistics. */
  | { name: "recap" }
  /** Local smart playlist (#383). */
  | { name: "smart"; list: SmartList }
  | { name: "settings"; section: SettingsSection }
  /** `track`: highlighted track (pasted link, #203). */
  | { name: "album"; id: number; track?: number }
  | { name: "artist"; id: number }
  /** Tracks where a person is credited (#429), opened from the credits. */
  | { name: "contributor"; id: number }
  | { name: "playlist"; uuid: string }
  /** Playlist folder of the library (#331). */
  | { name: "folder"; id: string }
  | { name: "mix"; id: string }
  /** Playlist from another service, to import (#328). */
  | { name: "import"; link: string }
  | { name: "radio"; seed: RadioSeed; id: number }
  /** Blind test (#434): setup, game in progress or score. */
  | { name: "blindTest" };

const MAX_HISTORY = 50;

export const nav = $state<{ stack: Route[] }>({ stack: [{ name: "home" }] });

export function current(): Route {
  return nav.stack[nav.stack.length - 1];
}

export function go(route: Route) {
  nav.stack = [...nav.stack, route].slice(-MAX_HISTORY);
}

/** Replaces the current page (tab change, menu entry). */
export function replace(route: Route) {
  nav.stack = [...nav.stack.slice(0, -1), route];
}

/** Opens a settings section: switches section if the page is already open. */
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
