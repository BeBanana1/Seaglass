// Entrées du menu contextuel d'un titre (#52). Chaque action passe par une commande du backend.

import { commands, type BlockTarget, type Track } from "./ipc";
import { fullTitle } from "./format";
import { isLiked, toggleLike } from "./stores/likes.svelte";
import { isSavedForLater, toggleLater } from "./stores/later.svelte";
import { go } from "./stores/nav.svelte";
import { openPlaylistPicker } from "./stores/playlistPicker.svelte";
import type { MenuEntry } from "./stores/menu.svelte";
import { pushToast, pushUndoToast } from "./stores/toasts.svelte";
import { openTrackInfo } from "./stores/trackInfo.svelte";
import { t } from "./i18n.svelte";

export interface TrackMenuOptions {
  /** Lire le titre dans son contexte (liste affichée). */
  onPlay?: () => void;
  /** Titre de la file manuelle : « Retirer » remplace les ajouts à la file. */
  onRemove?: () => void;
  /** Titre de la file manuelle qui peut monter / descendre d'un cran (#80). */
  onMoveUp?: () => void;
  onMoveDown?: () => void;
  /** « Lire ensuite » / « Ajouter à la file ». */
  queueable?: boolean;
  /** Titre d'une de vos playlists : « Retirer de cette playlist ». */
  onRemoveFromPlaylist?: () => void;
  /** Après un blocage confirmé (#334) : « En lecture » passe au titre suivant. */
  onBlocked?: () => void;
  /** Liste qui permet la sélection multiple (#332) : « Sélectionner ». */
  onSelect?: () => void;
  /** Track being played (#518): "Copy as image" renders the "Now playing" card. */
  onShareImage?: () => void;
}

/** Lien public du titre, à partager. */
export function trackLink(track: Track): string {
  return `https://tidal.com/browse/track/${track.id}`;
}

async function run(action: () => Promise<unknown>) {
  try {
    await action();
  } catch (e) {
    pushToast({ level: "error", message: String(e) });
  }
}

/** Bloque (#334), puis propose d'annuler : le blocage vaut sur tous les appareils. */
async function block(target: BlockTarget, message: string, onBlocked?: () => void) {
  try {
    await commands.setBlocked(target, true);
  } catch (e) {
    pushToast({ level: "error", message: String(e) });
    return;
  }
  onBlocked?.();
  pushUndoToast(message, () => run(() => commands.setBlocked(target, false)));
}

async function copyLink(track: Track) {
  await navigator.clipboard.writeText(trackLink(track));
  pushToast({ level: "info", message: t.common.linkCopied });
}

export function trackMenu(
  track: Track,
  {
    onPlay,
    onRemove,
    onMoveUp,
    onMoveDown,
    queueable = true,
    onRemoveFromPlaylist,
    onBlocked,
    onSelect,
    onShareImage,
  }: TrackMenuOptions,
): MenuEntry[] {
  const m = t.menu;
  const entries: MenuEntry[] = [];
  if (onPlay) entries.push({ label: m.play, icon: "play", action: onPlay });
  if (onSelect) entries.push({ label: m.select, icon: "check", action: onSelect });
  if (onMoveUp) entries.push({ label: m.moveUp, icon: "up", action: onMoveUp });
  if (onMoveDown) entries.push({ label: m.moveDown, icon: "down", action: onMoveDown });
  if (onRemove) {
    entries.push({ label: m.removeFromQueue, icon: "close", action: onRemove });
  } else if (queueable) {
    entries.push(
      { label: m.playNext, icon: "playNext", action: () => run(() => commands.playNext(track.id)) },
      { label: m.addToQueue, icon: "add", action: () => run(() => commands.addToQueue(track.id)) },
    );
  }
  if (entries.length > 0) entries.push("separator");

  const liked = isLiked(track.id);
  entries.push(
    {
      label: liked ? m.unlike : m.like,
      icon: liked ? "heartFilled" : "heart",
      action: () => toggleLike(track),
    },
    {
      label: m.addToPlaylist,
      icon: "playlistAdd",
      action: () => openPlaylistPicker({ trackIds: [track.id], label: fullTitle(track) }),
    },
  );
  const target = { kind: "track", id: track.id } as const;
  const saved = isSavedForLater(target);
  entries.push({
    label: saved ? m.removeFromLater : m.saveForLater,
    icon: saved ? "bookmarkFilled" : "bookmark",
    action: () => toggleLater(target),
  });
  if (onRemoveFromPlaylist) {
    entries.push({ label: m.removeFromPlaylist, icon: "remove", action: onRemoveFromPlaylist });
  }
  entries.push("separator");

  entries.push(
    {
      label: m.trackRadio,
      icon: "radio",
      action: () => go({ name: "radio", seed: "track", id: track.id }),
    },
    { label: m.goToAlbum, icon: "album", action: () => go({ name: "album", id: track.album.id }) },
  );
  // Une entrée par artiste : « Aller à l'artiste » seul serait ambigu sur un featuring.
  for (const artist of track.artists) {
    entries.push({
      label: track.artists.length > 1 ? m.goTo(artist.name) : m.goToArtist,
      icon: "artist",
      action: () => go({ name: "artist", id: artist.id }),
    });
  }

  entries.push(
    "separator",
    { label: m.trackInfo, icon: "info", action: () => openTrackInfo(track) },
    { label: m.copyLink, icon: "link", action: () => run(() => copyLink(track)) },
  );
  if (onShareImage) entries.push({ label: m.shareImage, icon: "image", action: onShareImage });
  entries.push("separator", {
    label: m.blockTrack,
    icon: "block",
    action: () =>
      block({ kind: "track", id: track.id }, m.trackBlocked(fullTitle(track)), onBlocked),
  });
  // Artiste principal seulement : un invité se bloque depuis un de ses propres titres.
  const main = track.artists[0];
  if (main) {
    entries.push({
      label: m.blockArtist(main.name),
      icon: "block",
      action: () => block({ kind: "artist", id: main.id }, m.artistBlocked(main.name), onBlocked),
    });
  }
  return entries;
}
