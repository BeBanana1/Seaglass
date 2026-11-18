// Filtre et tri d'une liste de titres déjà chargée (playlist, titres likés).
// Présentation uniquement : aucune requête, la liste reçue du backend n'est pas modifiée.

import { artistNames, fullTitle } from "./format";
import type { SortKey, TrackEntry, TrackSort } from "./ipc";
import { locale } from "./i18n.svelte";

export type { SortKey, TrackSort };

/** Titre affiché, avec sa position (à partir de 1) dans la liste d'origine. */
export interface ListedTrack {
  track: TrackEntry;
  position: number;
}

/** Minuscules sans accents : « Beyoncé » et « beyonce » se confondent. */
export function normalize(text: string): string {
  return text.normalize("NFD").replace(/\p{M}/gu, "").toLocaleLowerCase("fr");
}

// Tri alphabétique de la langue courante, recréé seulement quand elle change.
let collator: { locale: string; intl: Intl.Collator } | null = null;

function collate(a: string, b: string): number {
  const current = locale();
  if (collator?.locale !== current) {
    collator = {
      locale: current,
      intl: new Intl.Collator(current, { sensitivity: "base", numeric: true }),
    };
  }
  return collator.intl.compare(a, b);
}

// Texte de recherche de chaque titre, calculé une fois par objet reçu.
const haystacks = new WeakMap<TrackEntry, string>();

function haystack(track: TrackEntry): string {
  let text = haystacks.get(track);
  if (text === undefined) {
    text = normalize(`${fullTitle(track)} ${artistNames(track)} ${track.album.title}`);
    haystacks.set(track, text);
  }
  return text;
}

/** Ordre d'une playlist tant qu'aucun tri n'est mémorisé : celui de la playlist. */
export const PLAYLIST_DEFAULT_SORT: TrackSort = { key: "position", descending: false };
/** Titres likés : les plus récents d'abord. */
export const LIKED_DEFAULT_SORT: TrackSort = { key: "added", descending: true };

export function listTracks(tracks: TrackEntry[]): ListedTrack[] {
  return tracks.map((track, i) => ({ track, position: i + 1 }));
}

/** Garde les titres dont le titre, les artistes ou l'album contiennent chaque mot. */
export function filterTracks(list: ListedTrack[], query: string): ListedTrack[] {
  const terms = normalize(query).split(/\s+/).filter(Boolean);
  if (terms.length === 0) return list;
  return list.filter(({ track }) => {
    const text = haystack(track);
    return terms.every((term) => text.includes(term));
  });
}

/**
 * Contexte de lecture d'un clic sur le titre `index` de la liste affichée (#496) : toute la
 * liste triée, pas seulement les résultats du filtre, qui sert à trouver un titre. `start`
 * est la place du titre cliqué dans `sorted` (par entrée, pour les doublons d'une playlist).
 */
export function playContext(
  sorted: ListedTrack[],
  shown: ListedTrack[],
  index: number,
): { ids: TrackEntry["id"][]; start: number } {
  const start = sorted.indexOf(shown[index]);
  if (start < 0) return { ids: shown.map((l) => l.track.id), start: index };
  return { ids: sorted.map((l) => l.track.id), start };
}

function compare(a: TrackEntry, b: TrackEntry, key: Exclude<SortKey, "position" | "added">) {
  switch (key) {
    case "title":
      return collate(fullTitle(a), fullTitle(b));
    case "artist":
      return collate(artistNames(a), artistNames(b));
    case "album":
      return collate(a.album.title, b.album.title);
    case "duration":
      return a.duration - b.duration;
  }
}

/** Tri stable ; à égalité, l'ordre d'origine. Les titres sans date d'ajout restent en fin. */
export function sortTracks(list: ListedTrack[], sort: TrackSort): ListedTrack[] {
  const dir = sort.descending ? -1 : 1;
  return [...list].sort((a, b) => {
    let result: number;
    if (sort.key === "position") {
      result = a.position - b.position;
    } else if (sort.key === "added") {
      // `?? null` : des fixtures anciennes n'ont pas le champ.
      const [x, y] = [a.track.addedAt ?? null, b.track.addedAt ?? null];
      if (x === null || y === null) {
        if (x !== y) return x === null ? 1 : -1;
        result = 0;
      } else {
        result = x - y;
      }
    } else {
      result = compare(a.track, b.track, sort.key);
    }
    return dir * result || a.position - b.position;
  });
}
