// Pins (#10): label and page of each, for the sidebar and the driving mode (#193).

import { commands, type Pin, type PlaySource, type TrackEntry, type TrackId } from "./ipc";
import type { LibraryTab, Route } from "./stores/nav.svelte";
import { listSort } from "./stores/ui.svelte";
import {
  LIKED_DEFAULT_SORT,
  listTracks,
  PLAYLIST_DEFAULT_SORT,
  sortTracks,
  type TrackSort,
} from "./trackOrder";
import { t } from "./i18n.svelte";

/** Library tab of a pin that is neither a playlist nor a mix. */
export const pinTab = (pin: Pin): LibraryTab => (pin.kind === "later" ? "later" : "tracks");

export function pinRoute(pin: Pin): Route {
  switch (pin.kind) {
    case "playlist":
      return { name: "playlist", uuid: pin.uuid };
    case "mix":
      return { name: "mix", id: pin.id };
    case "smart":
      return { name: "smart", list: pin.list };
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
    case "smart":
      return t.views.smart.names[pin.list];
  }
}

/** "Played from" (#124) of a pin. */
export function pinSource(pin: Pin): PlaySource {
  switch (pin.kind) {
    case "playlist":
      return { kind: "playlist", uuid: pin.uuid, title: pin.title };
    case "mix":
      return { kind: "mix", id: pin.id, title: pin.title };
    case "smart":
      return { kind: "smart", list: pin.list };
    default:
      return { kind: pin.kind };
  }
}

/** In the order shown by the page: remembered sort of the list (#93), otherwise its default. */
const sorted = (tracks: TrackEntry[], list: string, fallback: TrackSort): TrackId[] =>
  sortTracks(listTracks(tracks), listSort(list, fallback)).map((l) => l.track.id);

/** Tracks that the page's Play button of the pin would play, in the same order. */
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
      return (await commands.saveForLater()).items.flatMap((item) =>
        item.kind === "track" ? [item.id] : [],
      );
    case "smart":
      return (await commands.smartList(pin.list)).map((track) => track.id);
  }
}

/** Driving mode (#193): plays the whole pinned source, like Play (or Shuffle) on its page. */
export async function playPin(pin: Pin, shuffle: boolean): Promise<void> {
  const ids = await pinTracks(pin);
  if (ids.length === 0) throw new Error(t.views.mix.empty);
  await commands.playTracks(ids, 0, shuffle, pinSource(pin));
}
