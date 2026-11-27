// Favorites (#4, #81, #204), mirror of the sets held by the backend: hearts of tracks,
// albums, playlists and mixes, "Follow" button of artists.

import { commands, events, type LikedIds, type LikeTarget, type Track, type TrackId } from "../ipc";
import { library, loadLibrary } from "./library.svelte";
import { pushToast, pushUndoToast } from "./toasts.svelte";
import { t } from "../i18n.svelte";

export const likes = $state<{
  tracks: Set<TrackId>;
  albums: Set<number>;
  artists: Set<number>;
  playlists: Set<string>;
  mixes: Set<string>;
  /** Writes in progress, by `targetKey`. */
  pending: Set<string>;
}>({
  tracks: new Set(),
  albums: new Set(),
  artists: new Set(),
  playlists: new Set(),
  mixes: new Set(),
  pending: new Set(),
});

function apply(ids: LikedIds) {
  likes.tracks = new Set(ids.tracks);
  likes.albums = new Set(ids.albums);
  likes.artists = new Set(ids.artists);
  likes.playlists = new Set(ids.playlists);
  likes.mixes = new Set(ids.mixes);
}

export function targetKey(target: LikeTarget): string {
  return target.kind === "playlist" ? `playlist:${target.uuid}` : `${target.kind}:${target.id}`;
}

export function isFavorite(target: LikeTarget): boolean {
  switch (target.kind) {
    case "track":
      return likes.tracks.has(target.id);
    case "album":
      return likes.albums.has(target.id);
    case "artist":
      return likes.artists.has(target.id);
    case "playlist":
      return likes.playlists.has(target.uuid);
    case "mix":
      return likes.mixes.has(target.id);
  }
}

export function isLiked(id: TrackId): boolean {
  return likes.tracks.has(id);
}

export function isPending(target: LikeTarget): boolean {
  return likes.pending.has(targetKey(target));
}

export async function initLikes() {
  await events.onLikes(apply);
}

/** On sign-in: first load (one request, the whole library). */
export async function loadLikes() {
  try {
    apply(await commands.likes());
  } catch (e) {
    pushToast({ level: "error", message: t.common.likesError(String(e)) });
  }
}

export function clearLikes() {
  apply({ tracks: [], albums: [], artists: [], playlists: [], mixes: [] });
}

/** Library tab that lists this kind of favorite. */
const TABS = { album: "albums", artist: "artists", playlist: "playlists", mix: "mixes" } as const;

/**
 * Adds to or removes from favorites; the state changes when the backend confirms (`likes`
 * event), then the matching library tab is reloaded if it was already loaded.
 */
export async function toggleFavorite(target: LikeTarget) {
  const key = targetKey(target);
  if (likes.pending.has(key)) return;
  const liked = !isFavorite(target);
  likes.pending = new Set(likes.pending).add(key);
  try {
    await commands.setLiked(target, liked);
    // Removal (#349): undoable in one click; the restored favorite gets a new date.
    if (!liked) pushUndoToast(t.common.unliked, () => void toggleFavorite(target));
    if (target.kind !== "track") {
      const tab = TABS[target.kind];
      if (library[tab].items) loadLibrary(tab, true);
    }
  } catch (e) {
    pushToast({ level: "error", message: String(e) });
  } finally {
    const pending = new Set(likes.pending);
    pending.delete(key);
    likes.pending = pending;
  }
}

export function toggleLike(track: Track) {
  return toggleFavorite({ kind: "track", id: track.id });
}
