// Library loaded on demand, tab by tab, then kept in memory.

import {
  commands,
  type AlbumSummary,
  type ArtistSummary,
  type LaterItem,
  type LibraryList,
  type MixSummary,
  type PlaylistSummary,
  type TrackEntry,
} from "../ipc";
import type { LibraryTab } from "./nav.svelte";
import { pushToast } from "./toasts.svelte";

interface Slot<T> {
  items: T[] | null;
  /** List cut at the loading cap (#91): only the most recent items are there. */
  truncated: boolean;
  /** Count announced by TIDAL, if known. */
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
  later: empty<LaterItem>(),
});

// Mixes come as a plain list (cursor, no cap); the rest as a `LibraryList`.
const loaders: Record<LibraryTab, () => Promise<unknown[] | LibraryList<unknown>>> = {
  playlists: commands.libraryPlaylists,
  mixes: commands.libraryMixes,
  albums: commands.libraryAlbums,
  artists: commands.libraryArtists,
  tracks: commands.libraryTracks,
  later: commands.saveForLater,
};

/** Loads the tab if it is not loaded yet (`force` to refresh).
 *
 * First load after launch (#181): the copy saved during the previous run shows at once,
 * then the whole list is read again and replaces it (the API does not say whether it
 * changed). An error on that read leaves the copy shown, with a toast. */
export async function loadLibrary(tab: LibraryTab, force = false) {
  const slot = library[tab];
  if (slot.loading || (slot.items && !force)) return;
  slot.loading = true;
  slot.error = null;
  try {
    // No copy on disk for mixes or for "Save for later".
    if (!slot.items && tab !== "mixes" && tab !== "later") {
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
