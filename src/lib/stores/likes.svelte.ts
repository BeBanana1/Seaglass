// Favoris (#4, #81), miroir des ensembles tenus par le backend : cœurs des titres, des
// albums et des playlists, bouton « Suivre » des artistes.

import { commands, events, type LikedIds, type LikeTarget, type Track, type TrackId } from "../ipc";
import { library, loadLibrary } from "./library.svelte";
import { pushToast } from "./toasts.svelte";
import { t } from "../i18n.svelte";

export const likes = $state<{
  tracks: Set<TrackId>;
  albums: Set<number>;
  artists: Set<number>;
  playlists: Set<string>;
  /** Écritures en cours, par `targetKey`. */
  pending: Set<string>;
}>({
  tracks: new Set(),
  albums: new Set(),
  artists: new Set(),
  playlists: new Set(),
  pending: new Set(),
});

function apply(ids: LikedIds) {
  likes.tracks = new Set(ids.tracks);
  likes.albums = new Set(ids.albums);
  likes.artists = new Set(ids.artists);
  likes.playlists = new Set(ids.playlists);
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

/** À la connexion : premier chargement (une requête, toute la bibliothèque). */
export async function loadLikes() {
  try {
    apply(await commands.likes());
  } catch (e) {
    pushToast({ level: "error", message: t.common.likesError(String(e)) });
  }
}

export function clearLikes() {
  apply({ tracks: [], albums: [], artists: [], playlists: [] });
}

/** Onglet de la bibliothèque qui liste ce type de favori. */
const TABS = { album: "albums", artist: "artists", playlist: "playlists" } as const;

/**
 * Ajout aux favoris ou retrait ; l'état change quand le backend confirme (événement
 * `likes`), puis l'onglet de bibliothèque concerné est rechargé s'il l'était déjà.
 */
export async function toggleFavorite(target: LikeTarget) {
  const key = targetKey(target);
  if (likes.pending.has(key)) return;
  const liked = !isFavorite(target);
  likes.pending = new Set(likes.pending).add(key);
  try {
    await commands.setLiked(target, liked);
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
