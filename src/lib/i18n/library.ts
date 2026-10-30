// Bibliothèque, épingles, filtre et tri des listes, ajout à une playlist.

import { num } from "./state.svelte";

export const fr = {
  title: "Bibliothèque",
  refresh: "Actualiser",
  tabs: { playlists: "Playlists", mixes: "Mixes", albums: "Albums", artists: "Artistes", tracks: "Titres" },
  empty: "Rien ici pour l'instant.",
  noMatch: (query: string) => `Aucun titre ne correspond à « ${query} ».`,
  likedTracks: "Titres likés",
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
  tabs: { playlists: "Playlists", mixes: "Mixes", albums: "Albums", artists: "Artists", tracks: "Tracks" },
  empty: "Nothing here yet.",
  noMatch: (query: string) => `No tracks match “${query}”.`,
  likedTracks: "Liked tracks",
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
