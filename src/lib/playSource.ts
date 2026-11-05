// « Joué depuis » (#124) : libellé et page d'une source de lecture renvoyée par le backend.

import type { PlaySource } from "./ipc";
import type { Route } from "./stores/nav.svelte";
import { t } from "./i18n.svelte";

export interface PlayedFrom {
  label: string;
  route: Route;
}

function fromSource(s: PlaySource): PlayedFrom {
  switch (s.kind) {
    case "album":
      return { label: s.title, route: { name: "album", id: s.id } };
    case "playlist":
      return { label: s.title, route: { name: "playlist", uuid: s.uuid } };
    case "mix":
      return { label: s.title, route: { name: "mix", id: s.id } };
    case "radio":
      return { label: s.title, route: { name: "radio", seed: s.seed, id: s.id } };
    case "artist":
      return { label: t.views.nowPlaying.popularBy(s.name), route: { name: "artist", id: s.id } };
    case "likedTracks":
      return { label: t.library.likedTracks, route: { name: "library", tab: "tracks" } };
    case "later":
      return { label: t.library.tabs.later, route: { name: "library", tab: "later" } };
    case "search":
      return { label: t.views.nowPlaying.searchFor(s.query), route: { name: "search" } };
  }
}

/** Libellé et page de la source du titre en cours (radio automatique comprise, #78). */
export function playedFrom(source: PlaySource | null): PlayedFrom | null {
  return source ? fromSource(source) : null;
}
