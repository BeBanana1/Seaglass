// Épingles (#10) : libellé et page de chacune, pour la barre latérale et le mode conduite (#193).

import { commands, type Pin, type PlaySource, type TrackEntry, type TrackId } from "./ipc";
import type { LibraryTab, Route } from "./stores/nav.svelte";
import { listSort } from "./stores/ui.svelte";
import { LIKED_DEFAULT_SORT, listTracks, PLAYLIST_DEFAULT_SORT, sortTracks, type TrackSort } from "./trackOrder";
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

/** « Joué depuis » (#124) d'une épingle. */
export function pinSource(pin: Pin): PlaySource {
  switch (pin.kind) {
    case "playlist":
      return { kind: "playlist", uuid: pin.uuid, title: pin.title };
    case "mix":
      return { kind: "mix", id: pin.id, title: pin.title };
    default:
      return { kind: pin.kind };
  }
}

/** Dans l'ordre affiché par la page : tri mémorisé de la liste (#93), sinon son tri par défaut. */
const sorted = (tracks: TrackEntry[], list: string, fallback: TrackSort): TrackId[] =>
  sortTracks(listTracks(tracks), listSort(list, fallback)).map((l) => l.track.id);

/** Titres que lirait le bouton Lecture de la page de l'épingle, dans le même ordre. */
async function pinTracks(pin: Pin): Promise<TrackId[]> {
  switch (pin.kind) {
    case "playlist": {
      const page = await commands.playlist(pin.uuid);
      return page ? sorted(page.tracks, pin.uuid, PLAYLIST_DEFAULT_SORT) : [];
    }
    case "likedTracks":
      return sorted((await commands.libraryTracks()).items, "likedTracks", LIKED_DEFAULT_SORT);
    case "mix":
      return (await commands.mix(pin.id)).tracks.map((track) => track.id);
    case "later":
      return (await commands.saveForLater()).items.flatMap((item) => (item.kind === "track" ? [item.id] : []));
  }
}

/** Mode conduite (#193) : lit toute la source épinglée, comme Lecture (ou Aléatoire) sur sa page. */
export async function playPin(pin: Pin, shuffle: boolean): Promise<void> {
  const ids = await pinTracks(pin);
  if (ids.length === 0) throw new Error(t.views.mix.empty);
  await commands.playTracks(ids, 0, shuffle, pinSource(pin));
}
