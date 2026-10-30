// Bibliothèque chargée à la demande, onglet par onglet, puis gardée en mémoire.

import {
  commands,
  type AlbumSummary,
  type ArtistSummary,
  type PlaylistSummary,
  type TrackEntry,
} from "../ipc";
import type { LibraryTab } from "./nav.svelte";

interface Slot<T> {
  items: T[] | null;
  error: string | null;
  loading: boolean;
}

const empty = <T>(): Slot<T> => ({ items: null, error: null, loading: false });

export const library = $state({
  playlists: empty<PlaylistSummary>(),
  albums: empty<AlbumSummary>(),
  artists: empty<ArtistSummary>(),
  tracks: empty<TrackEntry>(),
});

const loaders = {
  playlists: commands.libraryPlaylists,
  albums: commands.libraryAlbums,
  artists: commands.libraryArtists,
  tracks: commands.libraryTracks,
} as const;

/** Charge l'onglet s'il ne l'est pas déjà (`force` pour actualiser). */
export async function loadLibrary(tab: LibraryTab, force = false) {
  const slot = library[tab];
  if (slot.loading || (slot.items && !force)) return;
  slot.loading = true;
  slot.error = null;
  try {
    // Chaque onglet a son propre type d'éléments ; l'affectation reste cohérente par clé.
    (slot as Slot<unknown>).items = await loaders[tab]();
  } catch (e) {
    slot.error = String(e);
  } finally {
    slot.loading = false;
  }
}
