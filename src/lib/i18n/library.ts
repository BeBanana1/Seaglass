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
    if (tab === "playlists")
      return `Les ${num(shown)} playlists les plus récentes${of} sont affichées.`;
    const noun = { tracks: "titres likés", albums: "albums", artists: "artistes" }[tab];
    const text = `Les ${num(shown)} ${noun} les plus récents${of} sont affichés.`;
    return tab === "tracks"
      ? `${text} Le filtre, le tri et la lecture ne portent que sur eux.`
      : text;
  },
  likedTracks: "Titres likés",
  // « Plus tard » (#196).
  later: {
    empty:
      "Rien de côté. « Enregistrer pour plus tard » dans le menu d'un titre, d'un album ou d'une playlist l'ajoute ici.",
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

  // Statistiques d'écoute locales (#326).
  stats: {
    open: "Statistiques",
    title: "Statistiques d'écoute",
    periods: { week: "7 jours", month: "30 jours", year: "12 mois", all: "Tout" },
    plays: (n: number) => `${num(n)} ${n > 1 ? "écoutes" : "écoute"}`,
    time: (hours: number, minutes: number) =>
      hours > 0 ? `${num(hours)} h ${minutes} min` : `${minutes} min`,
    timeLabel: "Temps d'écoute",
    playsLabel: "Écoutes",
    unknown: (n: number) =>
      `${num(n)} ${n > 1 ? "écoutes antérieures" : "écoute antérieure"} à la 0.5.33 sans durée connue.`,
    tracks: "Titres",
    albums: "Albums",
    artists: "Artistes",
    empty:
      "Aucune écoute sur cette période. Un titre compte une fois joué à moitié (ou 4 minutes).",
    note: "Calculées sur cet appareil, à partir de l'historique d'écoute de Seaglass (avec celui de l'appareil lié, s'il y en a un) : rien n'est envoyé à TIDAL. Elles sont effacées avec l'historique (déconnexion, Réglages › Application).",
  },

  // Rétrospective du mois et de l'année (#417).
  recap: {
    open: "Rétrospective",
    title: "Rétrospective",
    modes: { month: "Mois", year: "Année" },
    year: (year: number) => `Année ${year}`,
    previous: "Période précédente",
    next: "Période suivante",
    periodLabel: "Période",
    days: (n: number) => `${num(n)} ${n > 1 ? "jours" : "jour"} d'écoute`,
    daysLabel: "Jours d'écoute",
    topTrack: "Titre de la période",
    topArtist: "Artiste de la période",
    discoveries: "Découvertes",
    newArtistsTitle: "Artistes découverts",
    newArtists: (n: number) => `${num(n)} ${n > 1 ? "nouveaux artistes" : "nouvel artiste"}`,
    newTracks: (n: number) => `${num(n)} ${n > 1 ? "nouveaux titres" : "nouveau titre"}`,
    habits: "Habitudes",
    weekdays: ["Lundi", "Mardi", "Mercredi", "Jeudi", "Vendredi", "Samedi", "Dimanche"],
    weekdaysShort: ["L", "M", "M", "J", "V", "S", "D"],
    topWeekday: (day: string) => `Le ${day.toLowerCase()}, surtout`,
    topHour: (hour: number) => `Vers ${hour} h`,
    hoursLabel: "Écoutes par heure",
    weekdaysLabel: "Écoutes par jour de la semaine",
    empty: "Pas encore de rétrospective : elle se remplit avec l'historique d'écoute de Seaglass.",
    note: "Calculée sur cet appareil, en heure locale. Les découvertes sont les artistes et titres jamais écoutés avant dans Seaglass.",
  },

  // Dossiers de playlists (#331).
  folders: {
    count: (n: number) => `${num(n)} ${n > 1 ? "playlists" : "playlist"}`,
    rename: "Renommer",
    renameLabel: "Nom du dossier",
    save: "Enregistrer",
    cancel: "Annuler",
    delete: "Supprimer le dossier…",
    deleteConfirm: "Supprimer ce dossier ? Ses playlists sont gardées, hors dossier.",
    deleteYes: "Supprimer",
    empty:
      "Ce dossier est vide. « Déplacer vers un dossier… » dans le menu d'une playlist en ajoute.",
    notFound: "Ce dossier n'existe plus.",
    pickerTitle: "Déplacer vers un dossier",
    out: "Hors dossier",
    current: "Dossier actuel",
    newFolder: "Nouveau dossier…",
    newName: "Nom du dossier",
    create: "Créer",
    moved: (name: string) => `Déplacée dans « ${name} »`,
    movedOut: "Sortie du dossier",
    created: (name: string) => `Dossier « ${name} » créé`,
    deleted: (name: string) => `Dossier « ${name} » supprimé`,
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
    const noun = {
      tracks: "liked tracks",
      albums: "albums",
      artists: "artists",
      playlists: "playlists",
    }[tab];
    const text = `Showing the ${num(shown)} most recent ${noun}${of}.`;
    return tab === "tracks" ? `${text} Filtering, sorting and playback only cover these.` : text;
  },
  likedTracks: "Liked tracks",
  later: {
    empty:
      "Nothing saved. “Save for later” in the menu of a track, album or playlist puts it here.",
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

  stats: {
    open: "Stats",
    title: "Listening stats",
    periods: { week: "7 days", month: "30 days", year: "12 months", all: "All time" },
    plays: (n: number) => `${num(n)} ${n === 1 ? "play" : "plays"}`,
    time: (hours: number, minutes: number) =>
      hours > 0 ? `${num(hours)} h ${minutes} min` : `${minutes} min`,
    timeLabel: "Listening time",
    playsLabel: "Plays",
    unknown: (n: number) =>
      `${num(n)} ${n === 1 ? "play" : "plays"} from before 0.5.33 with no known duration.`,
    tracks: "Tracks",
    albums: "Albums",
    artists: "Artists",
    empty: "No plays in this period. A track counts once half of it (or 4 minutes) has played.",
    note: "Computed on this device from Seaglass's listening history (including the linked device's, if any): nothing is sent to TIDAL. They are cleared along with the history (sign-out, Settings › App).",
  },

  recap: {
    open: "Recap",
    title: "Recap",
    modes: { month: "Month", year: "Year" },
    year: (year: number) => `Year ${year}`,
    previous: "Previous period",
    next: "Next period",
    periodLabel: "Period",
    days: (n: number) => `${num(n)} listening ${n === 1 ? "day" : "days"}`,
    daysLabel: "Listening days",
    topTrack: "Track of the period",
    topArtist: "Artist of the period",
    discoveries: "Discoveries",
    newArtistsTitle: "New artists",
    newArtists: (n: number) => `${num(n)} new ${n === 1 ? "artist" : "artists"}`,
    newTracks: (n: number) => `${num(n)} new ${n === 1 ? "track" : "tracks"}`,
    habits: "Habits",
    weekdays: ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"],
    weekdaysShort: ["M", "T", "W", "T", "F", "S", "S"],
    topWeekday: (day: string) => `Mostly on ${day}s`,
    topHour: (hour: number) => `Around ${hour}:00`,
    hoursLabel: "Plays by hour",
    weekdaysLabel: "Plays by day of the week",
    empty: "No recap yet: it fills up with Seaglass's listening history.",
    note: "Computed on this device, in local time. Discoveries are artists and tracks never played before in Seaglass.",
  },

  folders: {
    count: (n: number) => `${num(n)} ${n === 1 ? "playlist" : "playlists"}`,
    rename: "Rename",
    renameLabel: "Folder name",
    save: "Save",
    cancel: "Cancel",
    delete: "Delete folder…",
    deleteConfirm: "Delete this folder? Its playlists are kept, outside any folder.",
    deleteYes: "Delete",
    empty: "This folder is empty. “Move to folder…” in a playlist's menu adds some.",
    notFound: "This folder no longer exists.",
    pickerTitle: "Move to folder",
    out: "No folder",
    current: "Current folder",
    newFolder: "New folder…",
    newName: "Folder name",
    create: "Create",
    moved: (name: string) => `Moved to “${name}”`,
    movedOut: "Taken out of the folder",
    created: (name: string) => `Folder “${name}” created`,
    deleted: (name: string) => `Folder “${name}” deleted`,
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
