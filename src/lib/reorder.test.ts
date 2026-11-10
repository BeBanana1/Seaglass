import { describe, expect, it, vi } from "vitest";
import { keyboardMove, Reorder } from "./reorder.svelte";

/** Événement de glisser au-dessus d'une ligne de 20 px de haut posée à `top`. */
function over(clientY: number, top: number) {
  return {
    preventDefault: vi.fn(),
    dataTransfer: null,
    clientY,
    currentTarget: { getBoundingClientRect: () => ({ top, height: 20 }) },
  } as unknown as DragEvent;
}

const dropEvent = () => ({ preventDefault: vi.fn() }) as unknown as DragEvent;
const start = () => ({ dataTransfer: null }) as unknown as DragEvent;

/** Glisse la ligne `from` et la dépose sur la moitié haute (`before`) ou basse de la ligne `i`. */
function drag(from: number, i: number, before: boolean) {
  const onMove = vi.fn();
  const r = new Reorder(onMove);
  r.dragstart(start(), from);
  r.dragover(over(i * 20 + (before ? 5 : 15), i * 20), i);
  r.drop(dropEvent());
  return { r, onMove };
}

describe("Reorder", () => {
  it("place l'emplacement selon la moitié de la ligne survolée", () => {
    const r = new Reorder(() => {});
    r.dragstart(start(), 0);
    r.dragover(over(45, 40), 2);
    expect(r.slot).toBe(2);
    r.dragover(over(55, 40), 2);
    expect(r.slot).toBe(3);
  });

  it("vers le bas : tient compte de l'entrée retirée", () => {
    expect(drag(0, 2, false).onMove).toHaveBeenCalledWith(0, 2);
    expect(drag(0, 2, true).onMove).toHaveBeenCalledWith(0, 1);
  });

  it("vers le haut : position de l'emplacement", () => {
    expect(drag(3, 1, true).onMove).toHaveBeenCalledWith(3, 1);
    expect(drag(3, 1, false).onMove).toHaveBeenCalledWith(3, 2);
  });

  it("ne déplace rien autour de sa propre place, et se réinitialise", () => {
    const above = drag(2, 2, true);
    expect(above.onMove).not.toHaveBeenCalled();
    expect(drag(2, 2, false).onMove).not.toHaveBeenCalled();
    expect(drag(2, 1, false).onMove).not.toHaveBeenCalled();
    expect([above.r.from, above.r.slot]).toEqual([null, null]);
  });

  it("ignore le survol sans glisser en cours", () => {
    const r = new Reorder(() => {});
    const e = over(5, 0);
    r.dragover(e, 0);
    expect(r.slot).toBeNull();
    expect(e.preventDefault).not.toHaveBeenCalled();
  });

  it("n'affiche le repère que là où le dépôt changerait quelque chose", () => {
    const r = new Reorder(() => {});
    r.dragstart(start(), 2);
    for (const [slot, shown] of [
      [1, true],
      [2, false],
      [3, false],
      [4, true],
    ] as const) {
      r.slot = slot;
      expect(r.marker(slot)).toBe(shown);
    }
    expect(r.marker(0)).toBe(false);
  });
});

describe("keyboardMove", () => {
  const key = (key: string, altKey = true) => ({ key, altKey, preventDefault: vi.fn() }) as unknown as KeyboardEvent;

  it("monte et descend d'une ligne avec Alt", () => {
    expect(keyboardMove(key("ArrowUp"), 2, 5)).toBe(1);
    expect(keyboardMove(key("ArrowDown"), 2, 5)).toBe(3);
  });

  it("s'arrête aux bords de la liste", () => {
    expect(keyboardMove(key("ArrowUp"), 0, 5)).toBeNull();
    expect(keyboardMove(key("ArrowDown"), 4, 5)).toBeNull();
  });

  it("ignore les autres touches, sans bloquer leur effet", () => {
    const plain = key("ArrowUp", false);
    expect(keyboardMove(plain, 2, 5)).toBeNull();
    expect(plain.preventDefault).not.toHaveBeenCalled();
    expect(keyboardMove(key("Enter"), 2, 5)).toBeNull();
  });
});
