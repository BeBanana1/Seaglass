// Bibliothèque, épingles, filtre et tri des listes, ajout à une playlist.

import { num } from "./state.svelte";

/** Catégories de la bibliothèque chargées avec un plafond (#91). */
export type LimitedTab = "playlists" | "albums" | "artists" | "tracks";

export const fr = {
  title: "Bibliothèque",
  refresh: "Actualiser",
  tabs: {
    playlists: "Playlists",
    mixes: "Mixes",
    albums: "Albums",
    artists: "Artistes",
    tracks: "Titres",
    later: "Plus tard",
  },
  empty: "Rien ici pour l'instant.",
  noMatch: (query: string) => `Aucun titre ne correspond à « ${query} ».`,
  // Catégorie coupée au plafond de chargement (#91) ; `total` inconnu : `null`.
  truncated: (tab: LimitedTab, shown: number, total: number | null) => {
    const of = total !== null && total > shown ? ` sur ${num(total)}` : "";
    if (tab === "playlists") return `Les ${num(shown)} playlists les plus récentes${of} sont affichées.`;
    const noun = { tracks: "titres likés", albums: "albums", artists: "artistes" }[tab];
    const text = `Les ${num(shown)} ${noun} les plus récents${of} sont affichés.`;
    return tab === "tracks" ? `${text} Le filtre, le tri et la lecture ne portent que sur eux.` : text;
  },
  likedTracks: "Titres likés",
  // « Plus tard » (#196).
  later: {
    empty: "Rien de côté. « Enregistrer pour plus tard » dans le menu d'un titre, d'un album ou d'une playlist l'ajoute ici.",
    others: "Albums, artistes et playlists",
    saved: "Enregistré pour plus tard",
    removed: "Retiré de « Plus tard »",
    truncated: (shown: number) => `Les ${num(shown)} éléments les plus récents sont affichés.`,
  },
  pinned: "Épinglés",
  unpin: "Désépingler",
  unpinItem: (label: string) => `Désépingler ${label}`,

  filter: "Filtrer les titres",
  sortBy: "Trier par",
  descending: "Ordre décroissant",
  ascending: "Ordre croissant",
  sort: {
    position: "Ordre de la playlist",
    added: "Date d'ajout",
    title: "Titre",
    artist: "Artiste",
    album: "Album",
    duration: "Durée",
  },

  picker: {
    title: "Ajouter à une playlist",
    filter: "Filtrer vos playlists",
    newName: "Nom de la playlist",
    newNameLabel: "Nom de la nouvelle playlist",
    create: "Créer",
    newPlaylist: "Nouvelle playlist…",
    none: "Vous n'avez pas encore de playlist.",
    noMatch: (query: string) => `Aucune playlist ne correspond à « ${query} ».`,
    already: (where: string) => `Déjà dans « ${where} »`,
    addedOne: (where: string) => `Ajouté à « ${where} »`,
    addedMany: (added: number, where: string, skipped: number) =>
      `${num(added)} ${added > 1 ? "titres ajoutés" : "titre ajouté"} à « ${where} »` +
      (skipped > 0 ? ` (${num(skipped)} déjà ${skipped > 1 ? "présents" : "présent"})` : ""),
    created: (title: string) => `Playlist « ${title} » créée`,
  },
};

export const en: typeof fr = {
  title: "Library",
  refresh: "Refresh",
  tabs: {
    playlists: "Playlists",
    mixes: "Mixes",
    albums: "Albums",
    artists: "Artists",
    tracks: "Tracks",
    later: "Save for later",
  },
  empty: "Nothing here yet.",
  noMatch: (query: string) => `No tracks match “${query}”.`,
  truncated: (tab: LimitedTab, shown: number, total: number | null) => {
    const of = total !== null && total > shown ? ` out of ${num(total)}` : "";
    const noun = { tracks: "liked tracks", albums: "albums", artists: "artists", playlists: "playlists" }[tab];
    const text = `Showing the ${num(shown)} most recent ${noun}${of}.`;
    return tab === "tracks" ? `${text} Filtering, sorting and playback only cover these.` : text;
  },
  likedTracks: "Liked tracks",
  later: {
    empty: "Nothing saved. “Save for later” in the menu of a track, album or playlist puts it here.",
    others: "Albums, artists and playlists",
    saved: "Saved for later",
    removed: "Removed from Save for later",
    truncated: (shown: number) => `Showing the ${num(shown)} most recent items.`,
  },
  pinned: "Pinned",
  unpin: "Unpin",
  unpinItem: (label: string) => `Unpin ${label}`,

  filter: "Filter tracks",
  sortBy: "Sort by",
  descending: "Descending",
  ascending: "Ascending",
  sort: {
    position: "Playlist order",
    added: "Date added",
    title: "Title",
    artist: "Artist",
    album: "Album",
    duration: "Duration",
  },

  picker: {
    title: "Add to playlist",
    filter: "Filter your playlists",
    newName: "Playlist name",
    newNameLabel: "New playlist name",
    create: "Create",
    newPlaylist: "New playlist…",
    none: "You don't have any playlists yet.",
    noMatch: (query: string) => `No playlists match “${query}”.`,
    already: (where: string) => `Already in “${where}”`,
    addedOne: (where: string) => `Added to “${where}”`,
    addedMany: (added: number, where: string, skipped: number) =>
      `${num(added)} ${added === 1 ? "track" : "tracks"} added to “${where}”` +
      (skipped > 0 ? ` (${num(skipped)} already there)` : ""),
    created: (title: string) => `Playlist “${title}” created`,
  },
};
