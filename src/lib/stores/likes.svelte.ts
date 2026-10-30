// Titres likés (#4), miroir de l'ensemble tenu par le backend : affichage des cœurs.

import { commands, events, type Track, type TrackId } from "../ipc";
import { pushToast } from "./toasts.svelte";

export const likes = $state<{ ids: Set<TrackId>; pending: Set<TrackId> }>({
  ids: new Set(),
  pending: new Set(),
});

export function isLiked(id: TrackId): boolean {
  return likes.ids.has(id);
}

export async function initLikes() {
  await events.onLikedTracks((ids) => (likes.ids = new Set(ids)));
}

/** À la connexion : premier chargement (une requête, toute la bibliothèque). */
export async function loadLikes() {
  try {
    likes.ids = new Set(await commands.likedTracks());
  } catch (e) {
    pushToast({ level: "error", message: `Titres likés illisibles : ${e}` });
  }
}

export function clearLikes() {
  likes.ids = new Set();
}

/** Like ou retrait ; le cœur change quand le backend confirme (événement `liked-tracks`). */
export async function toggleLike(track: Track) {
  if (likes.pending.has(track.id)) return;
  const liked = !isLiked(track.id);
  likes.pending = new Set(likes.pending).add(track.id);
  try {
    await commands.setTrackLiked(track.id, liked);
  } catch (e) {
    pushToast({ level: "error", message: String(e) });
  } finally {
    const pending = new Set(likes.pending);
    pending.delete(track.id);
    likes.pending = pending;
  }
}
