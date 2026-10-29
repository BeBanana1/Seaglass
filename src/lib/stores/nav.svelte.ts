// Navigation interne : une pile de routes, sans URL (l'app n'a qu'une fenêtre).

export type LibraryTab = "playlists" | "albums" | "artists" | "tracks";

export type Route =
  | { name: "home" }
  | { name: "search" }
  | { name: "library"; tab: LibraryTab }
  | { name: "album"; id: number }
  | { name: "artist"; id: number }
  | { name: "playlist"; uuid: string };

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

export function back() {
  if (nav.stack.length > 1) nav.stack = nav.stack.slice(0, -1);
}

export function canGoBack(): boolean {
  return nav.stack.length > 1;
}
