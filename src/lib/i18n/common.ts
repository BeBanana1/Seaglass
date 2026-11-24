// Textes partagés : navigation, en-tête, actions génériques, compteurs.

import { num } from "./state.svelte";

export const fr = {
  /** Badge des titres et albums générés par IA (#369). */
  ai: "IA",
  nav: {
    home: "Accueil",
    feed: "Fil d'actualité",
    search: "Recherche",
    library: "Bibliothèque",
    settings: "Réglages",
  },
  /** Libellés courts de la navigation du bas (Android). */
  tabs: {
    home: "Accueil",
    likes: "Titres likés",
    search: "Recherche",
    library: "Bibliothèque",
    feed: "Actualité",
  },
  back: "Retour",
  backHint: "Retour (Alt+←)",
  searchHint: "Rechercher",
  settingsHint: "Réglages (Ctrl+,)",
  about: "À propos",
  logout: "Se déconnecter",
  loading: "Chargement…",
  close: "Fermer",
  /** Banner of the website demo (#28). */
  demo: {
    label: "Démo",
    text: "Catalogue inventé, aucun son",
    download: "Télécharger Seaglass",
  },
  cancel: "Annuler",
  undo: "Annuler",
  unliked: "Retiré des favoris",
  play: "Lecture",
  shuffle: "Aléatoire",
  linkCopied: "Lien copié",
  tracks: (n: number) => `${num(n)} ${n > 1 ? "titres" : "titre"}`,
  /** « 12 sur 340 titres » (liste filtrée). */
  tracksOf: (shown: number, total: number) =>
    `${num(shown)} sur ${num(total)} ${total > 1 ? "titres" : "titre"}`,
  likesError: (e: string) => `Titres likés illisibles : ${e}`,
};

export const en: typeof fr = {
  ai: "AI",
  nav: {
    home: "Home",
    feed: "Feed",
    search: "Search",
    library: "Library",
    settings: "Settings",
  },
  tabs: {
    home: "Home",
    likes: "Likes",
    search: "Search",
    library: "Library",
    feed: "Feed",
  },
  back: "Back",
  backHint: "Back (Alt+←)",
  searchHint: "Search",
  settingsHint: "Settings (Ctrl+,)",
  about: "About",
  logout: "Sign out",
  loading: "Loading…",
  close: "Close",
  demo: {
    label: "Demo",
    text: "Invented catalog, no sound",
    download: "Download Seaglass",
  },
  cancel: "Cancel",
  undo: "Undo",
  unliked: "Removed from favourites",
  play: "Play",
  shuffle: "Shuffle",
  linkCopied: "Link copied",
  tracks: (n: number) => `${num(n)} ${n === 1 ? "track" : "tracks"}`,
  tracksOf: (shown: number, total: number) =>
    `${num(shown)} of ${num(total)} ${total === 1 ? "track" : "tracks"}`,
  likesError: (e: string) => `Could not read liked tracks: ${e}`,
};
