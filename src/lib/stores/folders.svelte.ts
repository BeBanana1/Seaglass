// Playlist folders (#331): read with the Playlists tab, kept up to date after each write
// the backend confirms. With no folder, `items` is empty and nothing is shown.

import { commands, type PlaylistFolder, type PlaylistSummary } from "../ipc";
import { t } from "../i18n.svelte";
import { pushToast } from "./toasts.svelte";

export const folders = $state<{ items: PlaylistFolder[] | null; loading: boolean }>({
  items: null,
  loading: false,
});

/** Loads the folders if they are not loaded yet (`force` to read them again). An error leaves
 * the list as it is: the playlists stay shown flat, with no message. */
export async function loadFolders(force = false) {
  if (folders.loading || (folders.items && !force)) return;
  folders.loading = true;
  try {
    folders.items = await commands.playlistFolders();
  } catch (e) {
    console.warn("playlist folders unreadable", e);
  } finally {
    folders.loading = false;
  }
}

export function clearFolders() {
  folders.items = null;
}

/** Folder that holds this playlist, if any. */
export function folderOf(uuid: string): PlaylistFolder | undefined {
  return folders.items?.find((f) => f.playlists.includes(uuid));
}

export function folderById(id: string): PlaylistFolder | undefined {
  return folders.items?.find((f) => f.id === id);
}

/** Playlists of a folder, in the library's order; those no longer in it (deleted, no
 * longer followed) drop out on their own. */
export function folderPlaylists(folder: PlaylistFolder, all: PlaylistSummary[]): PlaylistSummary[] {
  const inside = new Set(folder.playlists);
  return all.filter((p) => inside.has(p.uuid));
}

/** Playlists outside any folder. */
export function rootPlaylists(all: PlaylistSummary[]): PlaylistSummary[] {
  const filed = new Set((folders.items ?? []).flatMap((f) => f.playlists));
  return filed.size === 0 ? all : all.filter((p) => !filed.has(p.uuid));
}

function sortByName(items: PlaylistFolder[]): PlaylistFolder[] {
  return [...items].sort((a, b) =>
    a.name.localeCompare(b.name, undefined, { sensitivity: "base" }),
  );
}

function error(e: unknown) {
  pushToast({ level: "error", message: String(e) });
}

/** Moves playlists into an existing folder, or out of their folder (`folderId` null). */
export async function movePlaylists(uuids: string[], folderId: string | null): Promise<boolean> {
  try {
    await commands.moveToFolder(uuids, folderId);
  } catch (e) {
    error(e);
    return false;
  }
  const moved = new Set(uuids);
  folders.items = (folders.items ?? []).map((f) => ({
    ...f,
    playlists: [...f.playlists.filter((u) => !moved.has(u)), ...(f.id === folderId ? uuids : [])],
  }));
  const target = folderId ? folderById(folderId) : undefined;
  pushToast({
    level: "info",
    message: target ? t.library.folders.moved(target.name) : t.library.folders.movedOut,
  });
  return true;
}

/** Creates a folder with these playlists; returns the folder, or `null` on failure. */
export async function createFolder(name: string, uuids: string[]): Promise<PlaylistFolder | null> {
  let created: PlaylistFolder;
  try {
    created = await commands.createFolder(name, uuids);
  } catch (e) {
    error(e);
    return null;
  }
  const moved = new Set(uuids);
  const others = (folders.items ?? []).map((f) => ({
    ...f,
    playlists: f.playlists.filter((u) => !moved.has(u)),
  }));
  folders.items = sortByName([...others, created]);
  pushToast({ level: "info", message: t.library.folders.created(created.name) });
  return created;
}

export async function renameFolder(id: string, name: string): Promise<boolean> {
  try {
    await commands.renameFolder(id, name);
  } catch (e) {
    error(e);
    return false;
  }
  const clean = name.trim();
  folders.items = sortByName(
    (folders.items ?? []).map((f) => (f.id === id ? { ...f, name: clean } : f)),
  );
  return true;
}

export async function deleteFolder(id: string): Promise<boolean> {
  const folder = folderById(id);
  try {
    await commands.deleteFolder(id);
  } catch (e) {
    error(e);
    return false;
  }
  folders.items = (folders.items ?? []).filter((f) => f.id !== id);
  if (folder) pushToast({ level: "info", message: t.library.folders.deleted(folder.name) });
  return true;
}

// "Move to folder…" dialog (components/FolderPicker.svelte).
export interface FolderPickerRequest {
  uuids: string[];
  /** What is being moved, for the header (the playlist's title). */
  label: string;
}

export const folderPicker = $state<{ request: FolderPickerRequest | null }>({ request: null });

export function openFolderPicker(request: FolderPickerRequest) {
  folderPicker.request = request;
  void loadFolders();
}

export function closeFolderPicker() {
  folderPicker.request = null;
}
