// Entrées du menu contextuel d'un titre (#52). Chaque action passe par une commande du backend.

import { commands, type Track } from "./ipc";
import { fullTitle } from "./format";
import { isLiked, toggleLike } from "./stores/likes.svelte";
import { go } from "./stores/nav.svelte";
import { openPlaylistPicker } from "./stores/playlistPicker.svelte";
import type { MenuEntry } from "./stores/menu.svelte";
import { pushToast } from "./stores/toasts.svelte";
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

async function copyLink(track: Track) {
  await navigator.clipboard.writeText(trackLink(track));
  pushToast({ level: "info", message: t.common.linkCopied });
}

export function trackMenu(
  track: Track,
  { onPlay, onRemove, onMoveUp, onMoveDown, queueable = true, onRemoveFromPlaylist }: TrackMenuOptions,
): MenuEntry[] {
  const m = t.menu;
  const entries: MenuEntry[] = [];
  if (onPlay) entries.push({ label: m.play, icon: "play", action: onPlay });
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
  if (onRemoveFromPlaylist) {
    entries.push({ label: m.removeFromPlaylist, icon: "remove", action: onRemoveFromPlaylist });
  }
  entries.push("separator");

  entries.push(
    { label: m.trackRadio, icon: "radio", action: () => go({ name: "radio", seed: "track", id: track.id }) },
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
  return entries;
}
