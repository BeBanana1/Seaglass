// « Plus tard » (Save for later, #196), miroir des identifiants tenus par le backend : sert
// aux entrées « Enregistrer pour plus tard » / « Retirer de Plus tard » des menus.

import { commands, events, type LikedIds, type LikeTarget } from "../ipc";
import { library, loadLibrary } from "./library.svelte";
import { pushToast } from "./toasts.svelte";
import { t } from "../i18n.svelte";

export const later = $state<{
  tracks: Set<number>;
  albums: Set<number>;
  artists: Set<number>;
  playlists: Set<string>;
  /** Écritures en cours, par clé `kind:id`. */
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

/** Écriture en cours pour cet élément (bouton estompé, #227). */
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

/** À la connexion. Une erreur n'est pas affichée : seuls les menus en dépendent, et la
 * liste se relit à l'ouverture de son onglet. */
export async function loadLater() {
  try {
    apply(await commands.laterIds());
  } catch (e) {
    console.warn("« Plus tard » illisible", e);
  }
}

export function clearLater() {
  apply({ tracks: [], albums: [], artists: [], playlists: [], mixes: [] });
}

/**
 * Ajoute ou retire ; l'état change quand le backend confirme (événement `later`), puis
 * l'onglet « Plus tard » est rechargé s'il l'était déjà.
 */
export async function toggleLater(target: LikeTarget) {
  if (target.kind === "mix") return;
  const k = key(target);
  if (later.pending.has(k)) return;
  const saved = !isSavedForLater(target);
  later.pending = new Set(later.pending).add(k);
  try {
    await commands.setSavedForLater(target, saved);
    pushToast({ level: "info", message: saved ? t.library.later.saved : t.library.later.removed });
    if (library.later.items) loadLibrary("later", true);
  } catch (e) {
    pushToast({ level: "error", message: String(e) });
  } finally {
    const pending = new Set(later.pending);
    pending.delete(k);
    later.pending = pending;
  }
}
