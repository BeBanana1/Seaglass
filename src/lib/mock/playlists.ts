// The user's playlists in the mock backend: pages, edits, folders, cleanup and the playlist
// cache.

import { allTracks, fx, longPlaylist, params, type Handlers } from "./core";
import { libPlaylists } from "./favorites";
import type {
  CleanupReport,
  ListOp,
  ListOpsResult,
  PlaylistFolder,
  PlaylistPage,
  PlaylistSummary,
  Track,
} from "../ipc";

// Playlist folders (#331): none by default (nothing must show), a demo folder with
// `?folders=1`.
let mockFolders: PlaylistFolder[] =
  params.get("folders") === "1"
    ? [
        {
          id: "f0000000-0000-4000-8000-000000000001",
          name: "Soirées",
          playlists: libPlaylists.slice(0, 2).map((p) => p.uuid),
        },
      ]
    : [];

function unfile(uuids: string[]) {
  mockFolders = mockFolders.map((f) => ({
    ...f,
    playlists: f.playlists.filter((u) => !uuids.includes(u)),
  }));
}

const own: PlaylistSummary[] = fx.library.playlists.filter((p) => p.mine);
if (params.has("mine") && !own.some((p) => p.uuid === fx.playlist.playlist.uuid)) {
  own.unshift({ ...fx.playlist.playlist, mine: true, editorial: false });
}
const ownPages = new Map<string, PlaylistPage>();

/** Mock "Clean up" report (#382): an unavailable track replaced, a duplicate edition, an
 *  unavailable track with no replacement (unchecked), after a short delay (`invoke`). */
function cleanupReport(tracks: Track[]): CleanupReport {
  const [a, b, c, d, e] = tracks;
  return a && b && c && d && e
    ? {
        scanned: tracks.length,
        items: [
          {
            kind: "unavailable",
            index: 1,
            track: b,
            replacement: e,
            suggested: true,
            op: { op: "replace", index: 1, trackId: b.id, with: e.id },
          },
          {
            kind: "duplicate",
            index: 2,
            track: c,
            keptIndex: 0,
            kept: a,
            sameTrack: false,
            suggested: true,
            op: { op: "remove", index: 2, trackId: c.id },
          },
          {
            kind: "unavailable",
            index: 3,
            track: d,
            replacement: null,
            suggested: false,
            op: { op: "remove", index: 3, trackId: d.id },
          },
        ],
      }
    : { scanned: tracks.length, items: [] };
}

/** Nothing changes: everything is reported applied, and undoable (one track "does not come back"). */
function cleanupResult(ops: ListOp[], undoing: boolean): ListOpsResult {
  return {
    applied: ops.length,
    skipped: undoing ? 1 : 0,
    undo: undoing ? [] : [...ops].reverse(),
    error: null,
  };
}

/** Page of one of your playlists, created on first access with the demo tracks. */
function ownPage(uuid: string): PlaylistPage | undefined {
  let page = ownPages.get(uuid);
  if (!page) {
    const summary = own.find((p) => p.uuid === uuid);
    if (!summary) return undefined;
    const start = fx.pages?.playlists.find((p) => p.playlist.uuid === uuid) ?? fx.playlist;
    page = { playlist: summary, tracks: start.tracks.map((t) => ({ ...t })), revision: null };
    ownPages.set(uuid, page);
  }
  page.playlist.numberOfTracks = page.tracks.length;
  return page;
}

function addToOwn(uuid: string, trackIds: number[]): number {
  const page = ownPage(uuid);
  if (!page) throw new Error("playlist introuvable");
  let added = 0;
  for (const id of trackIds) {
    const t = allTracks.get(id);
    if (!t || page.tracks.some((e) => e.id === id)) continue;
    page.tracks.push({ ...t, addedAt: Date.now() });
    added++;
  }
  page.playlist.numberOfTracks = page.tracks.length;
  return added;
}

const cachedPlaylists = new Map<string, PlaylistPage>();
/** Last track removed from a playlist, for the undo (#349). */
let removedFromPlaylist: PlaylistPage["tracks"][number] | null = null;
let removedMany: PlaylistPage["tracks"][number][] = [];

/** `?slow=<ms>`: latency of the `playlist` command, to see "Loading…" (#152). */
export const slowPlaylist = Math.max(0, Number(params.get("slow")) || 0);

/** Current page, with a revision derived from its tracks (the real backend's ETag). */
function currentPlaylist(uuid: string): PlaylistPage {
  const page =
    ownPage(uuid) ??
    longPlaylist ??
    fx.pages?.playlists.find((p) => p.playlist.uuid === uuid) ??
    fx.playlist;
  let hash = 5381;
  for (const t of page.tracks) hash = (Math.imul(hash, 33) ^ t.id) >>> 0;
  return { ...page, revision: `"${hash}-${page.tracks.length}"` };
}

export const playlistsHandlers: Handlers = {
  // Like the real backend (#152): `null` if nothing changed since `revision`, and a copy kept
  // for `cached_playlist` (cleared by the edits made in the app).
  playlist: ({ uuid, revision }) => {
    const page = currentPlaylist(String(uuid));
    cachedPlaylists.set(String(uuid), structuredClone(page));
    return page.revision === revision ? null : page;
  },
  cached_playlist: ({ uuid }) => cachedPlaylists.get(String(uuid)) ?? null,
  own_playlists: () => own.map((p) => ownPage(p.uuid)?.playlist ?? p),
  add_to_playlist: ({ uuid, trackIds }) => {
    cachedPlaylists.delete(String(uuid));
    return addToOwn(String(uuid), [...(trackIds as number[])]);
  },
  create_playlist: ({ title, trackIds }) => {
    const uuid = crypto.randomUUID();
    const summary: PlaylistSummary = {
      uuid,
      title: String(title).trim(),
      description: null,
      imageUrl: null,
      numberOfTracks: 0,
      duration: null,
      editorial: false,
      mine: true,
    };
    own.unshift(summary);
    ownPages.set(uuid, { playlist: summary, tracks: [], revision: null });
    addToOwn(uuid, [...(trackIds as number[])]);
    return uuid;
  },
  playlist_cleanup: ({ uuid }) => cleanupReport(ownPage(String(uuid))?.tracks ?? []),
  clean_playlist: ({ ops }) => cleanupResult(ops as ListOp[], false),
  undo_playlist_cleanup: ({ ops }) => cleanupResult(ops as ListOp[], true),
  liked_cleanup: () => cleanupReport(fx.library.tracks),
  clean_liked: ({ ops }) =>
    cleanupResult(
      ops as ListOp[],
      (ops as ListOp[]).some((o) => o.op === "insert"),
    ),
  remove_from_playlist: ({ uuid, index, trackId }) => {
    cachedPlaylists.delete(String(uuid));
    const page = ownPage(String(uuid));
    if (page?.tracks[Number(index)]?.id !== Number(trackId)) {
      throw new Error("la playlist a changé, rouvrez-la avant de retirer ce titre");
    }
    removedFromPlaylist = page.tracks.splice(Number(index), 1)[0] ?? null;
    page.playlist.numberOfTracks = page.tracks.length;
  },
  restore_to_playlist: ({ uuid, index, trackId }) => {
    cachedPlaylists.delete(String(uuid));
    const page = ownPage(String(uuid));
    const entry = removedFromPlaylist;
    if (!page || !entry || entry.id !== Number(trackId)) return false;
    page.tracks.splice(Number(index), 0, entry);
    page.playlist.numberOfTracks = page.tracks.length;
    removedFromPlaylist = null;
    return true;
  },
  // Multiple selection (#332): the removed positions are kept for the undo.
  remove_many_from_playlist: ({ uuid, items }) => {
    cachedPlaylists.delete(String(uuid));
    const page = ownPage(String(uuid));
    const list = [...(items as { index: number; trackId: number }[])].sort(
      (a, b) => b.index - a.index,
    );
    if (!page || list.some((i) => page.tracks[i.index]?.id !== i.trackId)) {
      throw new Error("la playlist a changé, rouvrez-la avant de la modifier");
    }
    removedMany = list.map((i) => page.tracks.splice(i.index, 1)[0]);
    page.playlist.numberOfTracks = page.tracks.length;
  },
  restore_many_to_playlist: ({ uuid, items }) => {
    cachedPlaylists.delete(String(uuid));
    const page = ownPage(String(uuid));
    if (!page) return 0;
    const list = [...(items as { index: number; trackId: number }[])].sort(
      (a, b) => a.index - b.index,
    );
    let restored = 0;
    for (const item of list) {
      const entry = removedMany.find((e) => e.id === item.trackId);
      if (!entry) continue;
      page.tracks.splice(item.index, 0, entry);
      restored++;
    }
    removedMany = [];
    page.playlist.numberOfTracks = page.tracks.length;
    return restored;
  },
  move_in_playlist: ({ uuid, from, to, trackId }) => {
    cachedPlaylists.delete(String(uuid));
    const page = ownPage(String(uuid));
    if (page?.tracks[Number(from)]?.id !== Number(trackId)) {
      throw new Error("la playlist a changé, rouvrez-la avant de la modifier");
    }
    const [moved] = page.tracks.splice(Number(from), 1);
    page.tracks.splice(Number(to), 0, moved);
  },
  edit_playlist: ({ uuid, title, description }) => {
    cachedPlaylists.delete(String(uuid));
    const page = ownPage(String(uuid));
    if (!page) throw new Error("playlist introuvable");
    page.playlist.title = String(title);
    page.playlist.description = String(description) || null;
    for (const list of [own, libPlaylists]) {
      const p = list.find((x) => x.uuid === uuid);
      if (p)
        Object.assign(p, { title: page.playlist.title, description: page.playlist.description });
    }
  },
  playlist_folders: () =>
    [...mockFolders]
      .sort((a, b) => a.name.localeCompare(b.name))
      .map((f) => ({ ...f, playlists: [...f.playlists] })),
  create_folder: ({ name, uuids }) => {
    const list = [...(uuids as string[])];
    unfile(list);
    const folder: PlaylistFolder = {
      id: crypto.randomUUID(),
      name: String(name).trim(),
      playlists: list,
    };
    mockFolders = [...mockFolders, folder];
    return { ...folder, playlists: [...list] };
  },
  rename_folder: ({ id, name }) => {
    mockFolders = mockFolders.map((f) => (f.id === id ? { ...f, name: String(name).trim() } : f));
  },
  move_to_folder: ({ uuids, folderId }) => {
    const list = [...(uuids as string[])];
    unfile(list);
    mockFolders = mockFolders.map((f) =>
      f.id === folderId ? { ...f, playlists: [...f.playlists, ...list] } : f,
    );
  },
  delete_folder: ({ id }) => {
    mockFolders = mockFolders.filter((f) => f.id !== id);
  },
  delete_playlist: ({ uuid }) => {
    cachedPlaylists.delete(String(uuid));
    ownPages.delete(String(uuid));
    for (const list of [own, libPlaylists]) {
      const i = list.findIndex((p) => p.uuid === uuid);
      if (i >= 0) list.splice(i, 1);
    }
  },
};
