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

/** Charge l'onglet s'il ne l'est pas déjà (`force` pour actualiser). */
export async function loadLibrary(tab: LibraryTab, force = false) {
  const slot = library[tab];
  if (slot.loading || (slot.items && !force)) return;
  slot.loading = true;
  slot.error = null;
  try {
    const result = await loaders[tab]();
    const list = Array.isArray(result) ? { items: result, truncated: false, total: null } : result;
    // Chaque onglet a son propre type d'éléments ; l'affectation reste cohérente par clé.
    const target = slot as Slot<unknown>;
    target.items = list.items;
    target.truncated = list.truncated;
    target.total = list.total;
  } catch (e) {
    slot.error = String(e);
  } finally {
    slot.loading = false;
  }
}
