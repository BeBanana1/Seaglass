// Multiple selection of tracks (#332). Only one list at a time holds the selection
// (`owner`); it is cleared when that list goes away (page change).
//
// Lists of thousands of tracks (#88): each track has its own reactive cell, created outside
// rendering, so checking a track only redraws its row. A `SvelteSet` is not enough: `has`
// on a missing item depends on the version of the whole set, so each addition would
// re-evaluate every unchecked row.

import type { Track, TrackId } from "../ipc";

/** A track list on screen, registered while it is mounted. */
export interface SelectionList {
  /** Tracks shown, in order (filtered and sorted list). */
  tracks: () => Track[];
  /** One of your playlists: removes the selected tracks. */
  removeFromPlaylist?: (ids: ReadonlySet<TrackId>) => Promise<void>;
}

/** Ids from `from` to `to` inclusive in the order of `ids`, in either direction. */
export function rangeIds(ids: TrackId[], from: TrackId, to: TrackId): TrackId[] {
  const a = ids.indexOf(from);
  const b = ids.indexOf(to);
  if (b < 0) return [];
  if (a < 0) return [to];
  return ids.slice(Math.min(a, b), Math.max(a, b) + 1);
}

class Selection {
  /** Number of selected tracks (0 = no selection). */
  count = $state(0);
  /** List that holds the selection. `raw`: compared by identity. */
  owner = $state.raw<SelectionList | null>(null);
  #ids = new Set<TrackId>();
  #cells = new Map<TrackId, { on: boolean }>();
  /** Last track checked or unchecked, starting point of Shift+click. */
  #anchor: TrackId | null = null;
  /** Mounted lists, the most recent last (target of Ctrl+A with no selection). */
  #lists: SelectionList[] = [];

  /** Changes when cells are created: only the rows still without a cell read it. */
  #created = $state(0);

  // Never called during rendering: state created in a render is not tracked there (Svelte
  // treats it as local to that render), so the row would not see its changes.
  #cell(id: TrackId) {
    const cell = this.#cells.get(id);
    if (cell) return cell;
    const fresh = $state({ on: false });
    this.#cells.set(id, fresh);
    this.#created++;
    return fresh;
  }

  /** Cells of the tracks of the list that holds the selection, created before they are read. */
  #prepare(ids: Iterable<TrackId>) {
    for (const id of ids) this.#cell(id);
  }

  #set(id: TrackId, on: boolean) {
    if (this.#ids.has(id) === on) return;
    if (on) this.#ids.add(id);
    else this.#ids.delete(id);
    this.#cell(id).on = on;
    this.count = this.#ids.size;
  }

  #take(list: SelectionList) {
    if (this.owner === list) return;
    this.clear();
    this.#prepare(list.tracks().map((t) => t.id));
    this.owner = list;
  }

  isSelected(list: SelectionList, id: TrackId): boolean {
    if (this.owner !== list) return false;
    const cell = this.#cells.get(id);
    if (cell) return cell.on;
    void this.#created;
    return false;
  }

  has(id: TrackId): boolean {
    return this.#ids.has(id);
  }

  register(list: SelectionList) {
    this.#lists.push(list);
  }

  unregister(list: SelectionList) {
    this.#lists = this.#lists.filter((l) => l !== list);
    if (this.owner === list) this.clear();
  }

  /** Ctrl+click, or a tap during the selection on Android. */
  toggle(list: SelectionList, id: TrackId) {
    this.#take(list);
    this.#set(id, !this.#ids.has(id));
    this.#anchor = id;
    if (this.count === 0) this.owner = null;
  }

  /** Shift+click: adds everything from the last clicked track to this one. */
  extend(list: SelectionList, id: TrackId) {
    const anchor = this.owner === list ? this.#anchor : null;
    this.#take(list);
    const ids = list.tracks().map((t) => t.id);
    for (const each of anchor === null ? [id] : rangeIds(ids, anchor, id)) this.#set(each, true);
    this.#anchor = id;
  }

  /** Ctrl+A: the list holding the selection, otherwise the last one shown. `false` with no list. */
  selectAll(list = this.owner ?? this.#lists.at(-1)): boolean {
    if (!list) return false;
    const tracks = list.tracks();
    if (tracks.length === 0) return false;
    this.#take(list);
    for (const track of tracks) this.#set(track.id, true);
    return true;
  }

  /** The list changed (filter, reload): the tracks no longer in it are unchecked. */
  prune(list: SelectionList, ids: TrackId[]) {
    if (this.owner !== list) return;
    this.#prepare(ids);
    const shown = new Set(ids);
    for (const id of [...this.#ids]) if (!shown.has(id)) this.#set(id, false);
    if (this.count === 0) this.clear();
  }

  /** Selected tracks in the list's order, with no duplicates. */
  tracks(): Track[] {
    const seen = new Set<TrackId>();
    return (this.owner?.tracks() ?? []).filter((t) => {
      if (!this.#ids.has(t.id) || seen.has(t.id)) return false;
      seen.add(t.id);
      return true;
    });
  }

  clear() {
    for (const id of this.#ids) this.#cell(id).on = false;
    this.#ids.clear();
    this.count = 0;
    this.owner = null;
    this.#anchor = null;
  }
}

export const selection = new Selection();
