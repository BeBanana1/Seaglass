// Dossiers de playlists (#331) : lus avec l'onglet Playlists, tenus à jour après chaque
// écriture confirmée par le backend. Sans dossier, `items` est vide et rien ne s'affiche.

import { commands, type PlaylistFolder, type PlaylistSummary } from "../ipc";
import { t } from "../i18n.svelte";
import { pushToast } from "./toasts.svelte";

export const folders = $state<{ items: PlaylistFolder[] | null; loading: boolean }>({
  items: null,
  loading: false,
});

/** Charge les dossiers s'ils ne le sont pas déjà (`force` pour relire). Une erreur laisse
 * la liste telle quelle : les playlists restent affichées à plat, sans message. */
export async function loadFolders(force = false) {
  if (folders.loading || (folders.items && !force)) return;
  folders.loading = true;
  try {
    folders.items = await commands.playlistFolders();
  } catch (e) {
    console.warn("dossiers de playlists illisibles", e);
  } finally {
    folders.loading = false;
  }
}

export function clearFolders() {
  folders.items = null;
}

/** Dossier qui contient cette playlist, s'il y en a un. */
export function folderOf(uuid: string): PlaylistFolder | undefined {
  return folders.items?.find((f) => f.playlists.includes(uuid));
}

export function folderById(id: string): PlaylistFolder | undefined {
  return folders.items?.find((f) => f.id === id);
}

/** Playlists d'un dossier, dans l'ordre de la bibliothèque ; celles qui n'y sont plus
 * (supprimée, plus suivie) disparaissent d'elles-mêmes. */
export function folderPlaylists(folder: PlaylistFolder, all: PlaylistSummary[]): PlaylistSummary[] {
  const inside = new Set(folder.playlists);
  return all.filter((p) => inside.has(p.uuid));
}

/** Playlists hors dossier. */
export function rootPlaylists(all: PlaylistSummary[]): PlaylistSummary[] {
  const filed = new Set((folders.items ?? []).flatMap((f) => f.playlists));
  return filed.size === 0 ? all : all.filter((p) => !filed.has(p.uuid));
}

function sortByName(items: PlaylistFolder[]): PlaylistFolder[] {
  return [...items].sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }));
}

function error(e: unknown) {
  pushToast({ level: "error", message: String(e) });
}

/** Range des playlists dans un dossier existant, ou les ressort (`folderId` nul). */
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
  pushToast({ level: "info", message: target ? t.library.folders.moved(target.name) : t.library.folders.movedOut });
  return true;
}

/** Crée un dossier avec ces playlists ; renvoie le dossier, ou `null` en cas d'échec. */
export async function createFolder(name: string, uuids: string[]): Promise<PlaylistFolder | null> {
  let created: PlaylistFolder;
  try {
    created = await commands.createFolder(name, uuids);
  } catch (e) {
    error(e);
    return null;
  }
  const moved = new Set(uuids);
  const others = (folders.items ?? []).map((f) => ({ ...f, playlists: f.playlists.filter((u) => !moved.has(u)) }));
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
  folders.items = sortByName((folders.items ?? []).map((f) => (f.id === id ? { ...f, name: clean } : f)));
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

// Fenêtre « Déplacer vers un dossier… » (components/FolderPicker.svelte).
export interface FolderPickerRequest {
  uuids: string[];
  /** Ce qui est rangé, pour l'en-tête (titre de la playlist). */
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
