// "Save for later" (#196), mirror of the ids held by the backend: used by the menus'
// "Save for later" / "Remove from Save for later" entries.

import { commands, events, type LikedIds, type LikeTarget } from "../ipc";
import { library, loadLibrary } from "./library.svelte";
import { pushToast, pushUndoToast } from "./toasts.svelte";
import { t } from "../i18n.svelte";

export const later = $state<{
  tracks: Set<number>;
  albums: Set<number>;
  artists: Set<number>;
  playlists: Set<string>;
  /** Writes in progress, by `kind:id` key. */
  pending: Set<string>;
}>({
  tracks: new Set(),
  albums: new Set(),
  artists: new Set(),
  playlists: new Set(),
  pending: new Set(),
});

function apply(ids: LikedIds) {
  later.tracks = new Set(ids.tracks);
  later.albums = new Set(ids.albums);
  later.artists = new Set(ids.artists);
  later.playlists = new Set(ids.playlists);
}

function key(target: LikeTarget): string {
  return target.kind === "playlist" ? `playlist:${target.uuid}` : `${target.kind}:${target.id}`;
}

/** Write in progress for this item (dimmed button, #227). */
export function isLaterPending(target: LikeTarget): boolean {
  return later.pending.has(key(target));
}

export function isSavedForLater(target: LikeTarget): boolean {
  switch (target.kind) {
    case "track":
      return later.tracks.has(target.id);
    case "album":
      return later.albums.has(target.id);
    case "artist":
      return later.artists.has(target.id);
    case "playlist":
      return later.playlists.has(target.uuid);
    case "mix":
      return false;
  }
}

export async function initLater() {
  await events.onLater(apply);
}

/** On sign-in. An error is not shown: only the menus depend on it, and the list is read
 * again when its tab opens. */
export async function loadLater() {
  try {
    apply(await commands.laterIds());
  } catch (e) {
    console.warn("Save for later list unreadable", e);
  }
}

export function clearLater() {
  apply({ tracks: [], albums: [], artists: [], playlists: [], mixes: [] });
}

/**
 * Adds or removes; the state changes when the backend confirms (`later` event), then the
 * "Save for later" tab is reloaded if it was already loaded.
 */
export async function toggleLater(target: LikeTarget) {
  if (target.kind === "mix") return;
  const k = key(target);
  if (later.pending.has(k)) return;
  const saved = !isSavedForLater(target);
  later.pending = new Set(later.pending).add(k);
  try {
    await commands.setSavedForLater(target, saved);
    if (saved) pushToast({ level: "info", message: t.library.later.saved });
    else pushUndoToast(t.library.later.removed, () => void toggleLater(target));
    if (library.later.items) loadLibrary("later", true);
  } catch (e) {
    pushToast({ level: "error", message: String(e) });
  } finally {
    const pending = new Set(later.pending);
    pending.delete(k);
    later.pending = pending;
  }
}
