// "Played from" (#124): label and page of a playback source returned by the backend.

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
    case "contributor":
      return {
        label: t.views.nowPlaying.creditsOf(s.name),
        route: { name: "contributor", id: s.id },
      };
    case "likedTracks":
      return { label: t.library.likedTracks, route: { name: "library", tab: "tracks" } };
    case "later":
      return { label: t.library.tabs.later, route: { name: "library", tab: "later" } };
    case "search":
      return { label: t.views.nowPlaying.searchFor(s.query), route: { name: "search" } };
    case "smart":
      return { label: t.views.smart.names[s.list], route: { name: "smart", list: s.list } };
    case "import":
      return { label: s.title, route: { name: "import", link: s.link } };
  }
}

/** Label and page of the source of the current track (automatic radio included, #78). */
export function playedFrom(source: PlaySource | null): PlayedFrom | null {
  return source ? fromSource(source) : null;
}
