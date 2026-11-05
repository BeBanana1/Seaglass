// Bibliothèque chargée à la demande, onglet par onglet, puis gardée en mémoire.

import {
  commands,
  type AlbumSummary,
  type ArtistSummary,
  type LibraryList,
  type MixSummary,
  type PlaylistSummary,
  type TrackEntry,
} from "../ipc";
import type { LibraryTab } from "./nav.svelte";
import { pushToast } from "./toasts.svelte";

interface Slot<T> {
  items: T[] | null;
  /** Liste coupée au plafond de chargement (#91) : seuls les plus récents sont là. */
  truncated: boolean;
  /** Nombre annoncé par TIDAL, s'il est connu. */
  total: number | null;
  error: string | null;
  loading: boolean;
}

const empty = <T>(): Slot<T> => ({
  items: null,
  truncated: false,
  total: null,
  error: null,
  loading: false,
});

export const library = $state({
  playlists: empty<PlaylistSummary>(),
  mixes: empty<MixSummary>(),
  albums: empty<AlbumSummary>(),
  artists: empty<ArtistSummary>(),
  tracks: empty<TrackEntry>(),
});

// Les mixes arrivent en simple liste (curseur, pas de plafond) ; le reste en `LibraryList`.
const loaders: Record<LibraryTab, () => Promise<unknown[] | LibraryList<unknown>>> = {
  playlists: commands.libraryPlaylists,
  mixes: commands.libraryMixes,
  albums: commands.libraryAlbums,
  artists: commands.libraryArtists,
  tracks: commands.libraryTracks,
};

/** Charge l'onglet s'il ne l'est pas déjà (`force` pour actualiser).
 *
 * Premier chargement du lancement (#181) : la copie enregistrée au lancement précédent
 * s'affiche aussitôt, puis la liste est relue en entier et la remplace (l'API ne dit pas
 * si elle a changé). Une erreur de relecture laisse la copie affichée, en toast. */
export async function loadLibrary(tab: LibraryTab, force = false) {
  const slot = library[tab];
  if (slot.loading || (slot.items && !force)) return;
  slot.loading = true;
  slot.error = null;
  try {
    if (!slot.items && tab !== "mixes") {
      const cached = await commands.cachedLibrary(tab).catch(() => null);
      if (cached && !slot.items) apply(slot, cached);
    }
    apply(slot, await loaders[tab]());
  } catch (e) {
    if (slot.items) pushToast({ level: "error", message: String(e) });
    else slot.error = String(e);
  } finally {
    slot.loading = false;
  }
}

function apply(slot: Slot<unknown>, result: unknown[] | LibraryList<unknown>) {
  const list = Array.isArray(result) ? { items: result, truncated: false, total: null } : result;
  slot.items = list.items;
  slot.truncated = list.truncated;
  slot.total = list.total;
}
