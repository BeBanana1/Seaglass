// Entrées du menu contextuel d'une carte (album, artiste, playlist, mix), #81.

import type { LikeTarget, MediaItem } from "./ipc";
import { isFavorite, toggleFavorite } from "./stores/likes.svelte";
import { go, type Route } from "./stores/nav.svelte";
import type { MenuEntry } from "./stores/menu.svelte";
import { pushToast } from "./stores/toasts.svelte";
import { t } from "./i18n.svelte";

export function cardRoute(item: MediaItem): Route {
  switch (item.kind) {
    case "album":
      return { name: "album", id: item.id };
    case "artist":
      return { name: "artist", id: item.id };
    case "mix":
      return { name: "mix", id: item.id };
    case "playlist":
      return { name: "playlist", uuid: item.uuid };
  }
}

/** Lien public, à partager. */
export function cardLink(item: MediaItem): string {
  const id = item.kind === "playlist" ? item.uuid : item.id;
  return `https://tidal.com/browse/${item.kind}/${id}`;
}

/** Favori possible : pas pour un mix (pas encore proposé) ni pour vos propres playlists. */
export function cardTarget(item: MediaItem): LikeTarget | null {
  switch (item.kind) {
    case "album":
      return { kind: "album", id: item.id };
    case "artist":
      return { kind: "artist", id: item.id };
    case "playlist":
      return item.mine ? null : { kind: "playlist", uuid: item.uuid };
    case "mix":
      return null;
  }
}

async function copyLink(item: MediaItem) {
  try {
    await navigator.clipboard.writeText(cardLink(item));
    pushToast({ level: "info", message: t.common.linkCopied });
  } catch (e) {
    pushToast({ level: "error", message: String(e) });
  }
}

export function cardMenu(item: MediaItem): MenuEntry[] {
  const m = t.menu;
  const entries: MenuEntry[] = [
    { label: m.open, icon: item.kind === "artist" ? "artist" : "album", action: () => go(cardRoute(item)) },
  ];

  const target = cardTarget(item);
  if (target) {
    const liked = isFavorite(target);
    const label =
      target.kind === "artist" ? (liked ? m.unfollow : m.follow) : liked ? m.removeFromLibrary : m.addToLibrary;
    entries.push({ label, icon: liked ? "heartFilled" : "heart", action: () => toggleFavorite(target) });
  }

  if (item.kind === "album") {
    entries.push("separator");
    for (const artist of item.artists) {
      entries.push({
        label: item.artists.length > 1 ? m.goTo(artist.name) : m.goToArtist,
        icon: "artist",
        action: () => go({ name: "artist", id: artist.id }),
      });
    }
  } else if (item.kind === "artist") {
    entries.push("separator", {
      label: m.artistRadio,
      icon: "radio",
      action: () => go({ name: "radio", seed: "artist", id: item.id }),
    });
  }

  entries.push("separator", { label: m.copyLink, icon: "link", action: () => copyLink(item) });
  return entries;
}
