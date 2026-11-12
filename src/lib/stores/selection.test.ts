import { afterEach, describe, expect, it } from "vitest";
import type { Track } from "../ipc";
import { rangeIds, selection, type SelectionList } from "./selection.svelte";

function list(ids: number[]): SelectionList & { ids: number[] } {
  const self = {
    ids,
    tracks: () => self.ids.map((id) => ({ id }) as Track),
  };
  return self;
}

afterEach(() => selection.clear());

describe("rangeIds", () => {
  const ids = [10, 20, 30, 40, 50];

  it("va dans les deux sens, bornes comprises", () => {
    expect(rangeIds(ids, 20, 40)).toEqual([20, 30, 40]);
    expect(rangeIds(ids, 40, 20)).toEqual([20, 30, 40]);
    expect(rangeIds(ids, 30, 30)).toEqual([30]);
  });

  it("sans point de départ affiché, prend seulement le titre cliqué", () => {
    expect(rangeIds(ids, 99, 30)).toEqual([30]);
    expect(rangeIds(ids, 10, 99)).toEqual([]);
  });
});

describe("selection", () => {
  it("coche et décoche, la dernière décoche termine la sélection", () => {
    const a = list([1, 2, 3]);
    selection.toggle(a, 2);
    expect(selection.count).toBe(1);
    expect(selection.isSelected(a, 2)).toBe(true);
    expect(selection.isSelected(a, 1)).toBe(false);
    selection.toggle(a, 2);
    expect(selection.count).toBe(0);
    expect(selection.owner).toBeNull();
  });

  it("Maj+clic étend depuis le dernier titre cliqué", () => {
    const a = list([1, 2, 3, 4, 5]);
    selection.toggle(a, 4);
    selection.extend(a, 2);
    expect(selection.tracks().map((t) => t.id)).toEqual([2, 3, 4]);
  });

  it("une autre liste reprend la sélection à zéro", () => {
    const a = list([1, 2]);
    const b = list([1, 3]);
    selection.toggle(a, 1);
    selection.toggle(b, 3);
    expect(selection.owner).toBe(b);
    expect(selection.isSelected(a, 1)).toBe(false);
    expect(selection.isSelected(b, 1)).toBe(false);
    expect(selection.count).toBe(1);
  });

  it("Ctrl+A vise la dernière liste inscrite, et la quitter vide la sélection", () => {
    const a = list([1, 2]);
    const b = list([3, 4, 5]);
    expect(selection.selectAll()).toBe(false);
    selection.register(a);
    selection.register(b);
    expect(selection.selectAll()).toBe(true);
    expect(selection.count).toBe(3);
    selection.unregister(b);
    expect(selection.count).toBe(0);
    selection.unregister(a);
  });

  it("garde l'ordre de la liste, sans doublon, et oublie les titres filtrés", () => {
    const a = list([3, 1, 2, 1]);
    selection.toggle(a, 1);
    selection.toggle(a, 3);
    expect(selection.tracks().map((t) => t.id)).toEqual([3, 1]);
    a.ids = [1, 2];
    selection.prune(a, a.ids);
    expect(selection.tracks().map((t) => t.id)).toEqual([1]);
    a.ids = [2];
    selection.prune(a, a.ids);
    expect(selection.owner).toBeNull();
  });
});
