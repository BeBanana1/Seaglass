// Sélection multiple de titres (#332). Une seule liste à la fois porte la sélection
// (`owner`) ; elle est vidée quand cette liste disparaît (changement de page).
//
// Listes de milliers de titres (#88) : chaque titre a sa propre cellule réactive, créée
// hors rendu, si bien que cocher un titre ne redessine que sa ligne. Un `SvelteSet` ne
// suffit pas : `has` d'un absent dépend de la version de tout l'ensemble, donc chaque ajout
// réévaluerait toutes les lignes non cochées.

import type { Track, TrackId } from "../ipc";

/** Une liste de titres affichée, inscrite tant qu'elle est montée. */
export interface SelectionList {
  /** Titres affichés, dans l'ordre (liste filtrée et triée). */
  tracks: () => Track[];
  /** Une de vos playlists : retire les titres sélectionnés. */
  removeFromPlaylist?: (ids: ReadonlySet<TrackId>) => Promise<void>;
}

/** Identifiants de `from` à `to` inclus dans l'ordre de `ids`, dans un sens ou dans l'autre. */
export function rangeIds(ids: TrackId[], from: TrackId, to: TrackId): TrackId[] {
  const a = ids.indexOf(from);
  const b = ids.indexOf(to);
  if (b < 0) return [];
  if (a < 0) return [to];
  return ids.slice(Math.min(a, b), Math.max(a, b) + 1);
}

class Selection {
  /** Nombre de titres sélectionnés (0 = pas de sélection). */
  count = $state(0);
  /** Liste qui porte la sélection. `raw` : comparée par identité. */
  owner = $state.raw<SelectionList | null>(null);
  #ids = new Set<TrackId>();
  #cells = new Map<TrackId, { on: boolean }>();
  /** Dernier titre coché ou décoché, point de départ de Maj+clic. */
  #anchor: TrackId | null = null;
  /** Listes montées, la plus récente en dernier (cible de Ctrl+A sans sélection). */
  #lists: SelectionList[] = [];

  /** Change quand des cellules sont créées : seules les lignes encore sans cellule le lisent. */
  #created = $state(0);

  // Jamais appelé pendant le rendu : un état créé dans un rendu n'y est pas suivi (Svelte
  // le traite comme local à ce rendu), la ligne ne verrait donc pas ses changements.
  #cell(id: TrackId) {
    const cell = this.#cells.get(id);
    if (cell) return cell;
    const fresh = $state({ on: false });
    this.#cells.set(id, fresh);
    this.#created++;
    return fresh;
  }

  /** Cellules des titres de la liste qui porte la sélection, créées avant leur lecture. */
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

  /** Ctrl+clic, ou un appui pendant la sélection sur Android. */
  toggle(list: SelectionList, id: TrackId) {
    this.#take(list);
    this.#set(id, !this.#ids.has(id));
    this.#anchor = id;
    if (this.count === 0) this.owner = null;
  }

  /** Maj+clic : ajoute tout ce qui va du dernier titre cliqué à celui-ci. */
  extend(list: SelectionList, id: TrackId) {
    const anchor = this.owner === list ? this.#anchor : null;
    this.#take(list);
    const ids = list.tracks().map((t) => t.id);
    for (const each of anchor === null ? [id] : rangeIds(ids, anchor, id)) this.#set(each, true);
    this.#anchor = id;
  }

  /** Ctrl+A : la liste qui porte la sélection, sinon la dernière affichée. `false` sans liste. */
  selectAll(list = this.owner ?? this.#lists.at(-1)): boolean {
    if (!list) return false;
    const tracks = list.tracks();
    if (tracks.length === 0) return false;
    this.#take(list);
    for (const track of tracks) this.#set(track.id, true);
    return true;
  }

  /** La liste a changé (filtre, rechargement) : les titres qui n'y sont plus sont décochés. */
  prune(list: SelectionList, ids: TrackId[]) {
    if (this.owner !== list) return;
    this.#prepare(ids);
    const shown = new Set(ids);
    for (const id of [...this.#ids]) if (!shown.has(id)) this.#set(id, false);
    if (this.count === 0) this.clear();
  }

  /** Titres sélectionnés dans l'ordre de la liste, sans doublon. */
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
