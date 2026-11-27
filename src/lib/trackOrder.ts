// Filter and sort of an already loaded list of tracks (playlist, liked tracks).
// Presentation only: no request, the list received from the backend is not modified.

import { artistNames, fullTitle } from "./format";
import type { SortKey, TrackEntry, TrackSort } from "./ipc";
import { locale } from "./i18n.svelte";

export type { SortKey, TrackSort };

/** Displayed track, with its position (from 1) in the original list. */
export interface ListedTrack {
  track: TrackEntry;
  position: number;
}

/** Lowercase without accents: "Beyoncé" and "beyonce" are conflated. */
export function normalize(text: string): string {
  return text.normalize("NFD").replace(/\p{M}/gu, "").toLocaleLowerCase("fr");
}

// Alphabetical sort of the current language, recreated only when it changes.
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

// Search text of each track, computed once per received object.
const haystacks = new WeakMap<TrackEntry, string>();

function haystack(track: TrackEntry): string {
  let text = haystacks.get(track);
  if (text === undefined) {
    text = normalize(`${fullTitle(track)} ${artistNames(track)} ${track.album.title}`);
    haystacks.set(track, text);
  }
  return text;
}

/** Order of a playlist while no sort is remembered: the playlist's own. */
export const PLAYLIST_DEFAULT_SORT: TrackSort = { key: "position", descending: false };
/** Liked tracks: most recent first. */
export const LIKED_DEFAULT_SORT: TrackSort = { key: "added", descending: true };

export function listTracks(tracks: TrackEntry[]): ListedTrack[] {
  return tracks.map((track, i) => ({ track, position: i + 1 }));
}

/** Keeps the tracks whose title, artists or album contain every word. */
export function filterTracks(list: ListedTrack[], query: string): ListedTrack[] {
  const terms = normalize(query).split(/\s+/).filter(Boolean);
  if (terms.length === 0) return list;
  return list.filter(({ track }) => {
    const text = haystack(track);
    return terms.every((term) => text.includes(term));
  });
}

/**
 * Playback context of a click on track `index` of the displayed list (#496): the whole
 * sorted list, not only the filter results, which serves to find a track. `start`
 * is the place of the clicked track in `sorted` (per entry, for duplicates in a playlist).
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

/** Stable sort; on a tie, the original order. Tracks without an added date stay at the end. */
export function sortTracks(list: ListedTrack[], sort: TrackSort): ListedTrack[] {
  const dir = sort.descending ? -1 : 1;
  return [...list].sort((a, b) => {
    let result: number;
    if (sort.key === "position") {
      result = a.position - b.position;
    } else if (sort.key === "added") {
      // `?? null`: old fixtures do not have the field.
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
