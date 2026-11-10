// Épingles (#10) : libellé et page de chacune, pour la barre latérale et le mode conduite (#193).

import type { Pin } from "./ipc";
import type { LibraryTab, Route } from "./stores/nav.svelte";
import { t } from "./i18n.svelte";

/** Onglet de la bibliothèque d'une épingle qui n'est ni une playlist ni un mix. */
export const pinTab = (pin: Pin): LibraryTab => (pin.kind === "later" ? "later" : "tracks");

export function pinRoute(pin: Pin): Route {
  switch (pin.kind) {
    case "playlist":
      return { name: "playlist", uuid: pin.uuid };
    case "mix":
      return { name: "mix", id: pin.id };
    default:
      return { name: "library", tab: pinTab(pin) };
  }
}

export function pinLabel(pin: Pin): string {
  switch (pin.kind) {
    case "playlist":
      return pin.title || t.format.playlist;
    case "mix":
      return pin.title || t.format.mix;
    case "later":
      return t.library.tabs.later;
    case "likedTracks":
      return t.library.likedTracks;
  }
}
