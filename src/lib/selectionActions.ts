// Actions sur la sélection multiple (#332), partagées par la barre d'actions et le menu
// contextuel d'un titre sélectionné. Chaque action vide la sélection une fois lancée.

import { commands, type TrackId } from "./ipc";
import { isLiked } from "./stores/likes.svelte";
import type { MenuEntry } from "./stores/menu.svelte";
import { openPlaylistPicker } from "./stores/playlistPicker.svelte";
import { selection } from "./stores/selection.svelte";
import { pushToast, pushUndoToast } from "./stores/toasts.svelte";
import { t } from "./i18n.svelte";

function ids(): TrackId[] {
  return selection.tracks().map((track) => track.id);
}

async function run(action: () => Promise<unknown>) {
  try {
    await action();
  } catch (e) {
    pushToast({ level: "error", message: String(e) });
  }
}

export function enqueueSelection(next: boolean) {
  const picked = ids();
  selection.clear();
  if (picked.length === 0) return;
  return run(async () => {
    await commands.enqueueTracks(picked, next);
    pushToast({ level: "info", message: t.menu.selection.queued(picked.length) });
  });
}

export function addSelectionToPlaylist() {
  const picked = ids();
  selection.clear();
  if (picked.length === 0) return;
  openPlaylistPicker({ trackIds: picked, label: t.menu.selection.label(picked.length) });
}

/** Tous déjà likés : on les retire (annulable), sinon on like ceux qui ne le sont pas. */
export function allSelectedLiked(): boolean {
  const picked = ids();
  return picked.length > 0 && picked.every(isLiked);
}

export function likeSelection() {
  const picked = ids();
  selection.clear();
  if (picked.length === 0) return;
  if (picked.every(isLiked)) {
    return run(async () => {
      await commands.setTracksLiked(picked, false);
      pushUndoToast(t.menu.selection.unliked(picked.length), () =>
        run(() => commands.setTracksLiked(picked, true)),
      );
    });
  }
  const missing = picked.filter((id) => !isLiked(id));
  return run(async () => {
    await commands.setTracksLiked(missing, true);
    pushToast({ level: "info", message: t.menu.selection.liked(missing.length) });
  });
}

export function canRemoveSelection(): boolean {
  return !!selection.owner?.removeFromPlaylist;
}

export function removeSelection() {
  const remove = selection.owner?.removeFromPlaylist;
  const picked = new Set(ids());
  selection.clear();
  if (!remove || picked.size === 0) return;
  return remove(picked);
}

/** Menu d'un titre sélectionné (clic droit, appui long, « … »). */
export function selectionMenu(): MenuEntry[] {
  const m = t.menu;
  const liked = allSelectedLiked();
  const entries: MenuEntry[] = [
    { label: m.playNext, icon: "playNext", action: () => enqueueSelection(true) },
    { label: m.addToQueue, icon: "add", action: () => enqueueSelection(false) },
    "separator",
    {
      label: liked ? m.selection.unlikeAll : m.selection.likeAll,
      icon: liked ? "heartFilled" : "heart",
      action: likeSelection,
    },
    { label: m.addToPlaylist, icon: "playlistAdd", action: addSelectionToPlaylist },
  ];
  if (canRemoveSelection()) {
    entries.push({ label: m.removeFromPlaylist, icon: "remove", action: removeSelection });
  }
  entries.push(
    "separator",
    { label: m.selection.selectAll, icon: "check", action: () => selection.selectAll() },
    { label: m.selection.clear, icon: "close", action: () => selection.clear() },
  );
  return entries;
}
