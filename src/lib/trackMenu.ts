// Entrées du menu contextuel d'un titre (#52). Chaque action passe par une commande du backend.

import { commands, type Track } from "./ipc";
import { fullTitle } from "./format";
import { isLiked, toggleLike } from "./stores/likes.svelte";
import { go } from "./stores/nav.svelte";
import { openPlaylistPicker } from "./stores/playlistPicker.svelte";
import type { MenuEntry } from "./stores/menu.svelte";
import { pushToast } from "./stores/toasts.svelte";

export interface TrackMenuOptions {
  /** Lire le titre dans son contexte (liste affichée). */
  onPlay?: () => void;
  /** Titre de la file manuelle : « Retirer » remplace les ajouts à la file. */
  onRemove?: () => void;
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
  pushToast({ level: "info", message: "Lien copié" });
}

export function trackMenu(
  track: Track,
  { onPlay, onRemove, queueable = true, onRemoveFromPlaylist }: TrackMenuOptions,
): MenuEntry[] {
  const entries: MenuEntry[] = [];
  if (onPlay) entries.push({ label: "Lire", icon: "play", action: onPlay });
  if (onRemove) {
    entries.push({ label: "Retirer de la file", icon: "close", action: onRemove });
  } else if (queueable) {
    entries.push(
      { label: "Lire ensuite", icon: "playNext", action: () => run(() => commands.playNext(track.id)) },
      { label: "Ajouter à la file", icon: "add", action: () => run(() => commands.addToQueue(track.id)) },
    );
  }
  if (entries.length > 0) entries.push("separator");

  const liked = isLiked(track.id);
  entries.push(
    {
      label: liked ? "Retirer des titres likés" : "Ajouter aux titres likés",
      icon: liked ? "heartFilled" : "heart",
      action: () => toggleLike(track),
    },
    {
      label: "Ajouter à une playlist…",
      icon: "playlistAdd",
      action: () => openPlaylistPicker({ trackIds: [track.id], label: fullTitle(track) }),
    },
  );
  if (onRemoveFromPlaylist) {
    entries.push({ label: "Retirer de cette playlist", icon: "remove", action: onRemoveFromPlaylist });
  }
  entries.push("separator");

  entries.push(
    { label: "Radio du titre", icon: "radio", action: () => go({ name: "radio", seed: "track", id: track.id }) },
    { label: "Aller à l'album", icon: "album", action: () => go({ name: "album", id: track.album.id }) },
  );
  // Une entrée par artiste : « Aller à l'artiste » seul serait ambigu sur un featuring.
  for (const artist of track.artists) {
    entries.push({
      label: track.artists.length > 1 ? `Aller à ${artist.name}` : "Aller à l'artiste",
      icon: "artist",
      action: () => go({ name: "artist", id: artist.id }),
    });
  }

  entries.push("separator", { label: "Copier le lien", icon: "link", action: () => run(() => copyLink(track)) });
  return entries;
}
