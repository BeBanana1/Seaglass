import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Reorder } from "./reorder.svelte";
import { touchReorder } from "./touch-reorder";

// Liste factice, sans DOM simulé : un conteneur qui défile (haut 0, bas 600) et douze lignes
// de 50 px posées à partir de 200 px dans le contenu (jusqu'à 800, plus bas que l'écran).
// La ligne saisie est la deuxième.
const ROW = 50;
const TOP = 200;
const COUNT = 12;

class FakeScroller {
  scrollTop = 0;
  scrollHeight = 2000;
  clientHeight = 600;
  parentElement = null;
  getBoundingClientRect() {
    return { top: 0, bottom: 600 };
  }
}

class FakeRow extends EventTarget {
  readonly classes = new Set<string>();
  readonly vars = new Map<string, string>();

  classList = {
    add: (name: string) => this.classes.add(name),
    remove: (name: string) => this.classes.delete(name),
  };

  style = {
    setProperty: (name: string, value: string) => this.vars.set(name, value),
    removeProperty: (name: string) => this.vars.delete(name),
  };

  constructor(
    readonly index: number,
    readonly parentElement: FakeScroller,
  ) {
    super();
  }

  closest() {
    return null;
  }

  /** Position à l'écran : suit le défilement et, pour la ligne saisie, son `--drag-y`. */
  getBoundingClientRect() {
    const shift = parseFloat(this.vars.get("--drag-y") ?? "0");
    const top = TOP + this.index * ROW - this.parentElement.scrollTop + shift;
    return { x: 0, y: top, top, width: 300, height: ROW };
  }
}

let frames: FrameRequestCallback[] = [];
const vibrate = vi.fn();

/** Joue les images d'animation en attente (défilement automatique). */
function nextFrame() {
  const pending = frames;
  frames = [];
  pending.forEach((cb) => cb(0));
}

function pointer(type: string, clientY: number, extra: Record<string, unknown> = {}) {
  return Object.assign(new Event(type, { cancelable: true }), {
    pointerId: 1,
    isPrimary: true,
    pointerType: "touch",
    clientX: 150,
    clientY,
    ...extra,
  });
}

let scroller: FakeScroller;
let rows: FakeRow[];
let row: FakeRow;
let onMove: ReturnType<typeof vi.fn<(from: number, to: number) => void>>;
let reorder: Reorder;
let action: ReturnType<typeof touchReorder>;

/** Remplace la liste par `count` lignes (la saisie reste sur la deuxième). */
function setRows(count: number) {
  action.destroy();
  rows = Array.from({ length: count }, (_, i) => new FakeRow(i, scroller));
  row = rows[1];
  action = touchReorder(row as unknown as HTMLElement, {
    enabled: true,
    index: 1,
    reorder,
    items: () => rows as unknown as HTMLElement[],
  });
}

/** Appui long sur la ligne saisie (au milieu, y = 275). */
function hold(y = 275) {
  row.dispatchEvent(pointer("pointerdown", y));
  vi.advanceTimersByTime(350);
}

function dispatch(type: string) {
  const e = new Event(type, { cancelable: true });
  row.dispatchEvent(e);
  return e.defaultPrevented;
}

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  frames = [];
  vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => frames.push(cb));
  vi.stubGlobal("cancelAnimationFrame", () => {
    frames = [];
  });
  vi.stubGlobal("getComputedStyle", (el: unknown) => ({ overflowY: el instanceof FakeScroller ? "auto" : "visible" }));
  vi.stubGlobal("document", { scrollingElement: null, documentElement: null });
  vi.stubGlobal("window", { innerHeight: 800 });
  vi.stubGlobal("navigator", { vibrate });
  vi.stubGlobal(
    "DOMRect",
    class {
      top: number;
      constructor(
        readonly x: number,
        readonly y: number,
        readonly width: number,
        readonly height: number,
      ) {
        this.top = y;
      }
    },
  );
  scroller = new FakeScroller();
  rows = Array.from({ length: COUNT }, (_, i) => new FakeRow(i, scroller));
  row = rows[1];
  onMove = vi.fn<(from: number, to: number) => void>();
  reorder = new Reorder(onMove);
  action = touchReorder(row as unknown as HTMLElement, {
    enabled: true,
    index: 1,
    reorder,
    items: () => rows as unknown as HTMLElement[],
  });
});

afterEach(() => {
  action.destroy();
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vibrate.mockClear();
});

describe("touchReorder", () => {
  it("saisit la ligne après un appui long, avec une vibration", () => {
    row.dispatchEvent(pointer("pointerdown", 275));
    vi.advanceTimersByTime(349);
    expect(reorder.from).toBeNull();
    vi.advanceTimersByTime(1);
    expect(reorder.from).toBe(1);
    expect(row.classes.has("touch-dragging")).toBe(true);
    expect(vibrate).toHaveBeenCalledOnce();
    // Sans bouger, la ligne reste à sa place : le dépôt ne change rien.
    expect(reorder.slot).toBe(2);
    row.dispatchEvent(pointer("pointerup", 275));
    expect(onMove).not.toHaveBeenCalled();
  });

  it("déplace vers le bas : la ligne suit le doigt, situé d'après sa place d'origine", () => {
    hold();
    row.dispatchEvent(pointer("pointermove", 330));
    expect(row.vars.get("--drag-y")).toBe("55px");
    expect(reorder.slot).toBe(3);
    row.dispatchEvent(pointer("pointerup", 330));
    expect(onMove).toHaveBeenCalledWith(1, 2);
    expect(row.classes.has("touch-dragging")).toBe(false);
    expect(row.vars.has("--drag-y")).toBe(false);
    expect([reorder.from, reorder.slot]).toEqual([null, null]);
  });

  it("déplace vers le haut", () => {
    hold();
    // Au-dessus de son propre milieu d'origine : emplacement juste avant elle (sans effet).
    row.dispatchEvent(pointer("pointermove", 260));
    expect(reorder.slot).toBe(1);
    row.dispatchEvent(pointer("pointermove", 210));
    expect(reorder.slot).toBe(0);
    row.dispatchEvent(pointer("pointerup", 210));
    expect(onMove).toHaveBeenCalledWith(1, 0);
  });

  it("tolère un léger mouvement pendant l'appui", () => {
    row.dispatchEvent(pointer("pointerdown", 275));
    row.dispatchEvent(pointer("pointermove", 282));
    vi.advanceTimersByTime(350);
    expect(reorder.from).toBe(1);
  });

  it("rend la main au défilement si le doigt bouge avant la saisie", () => {
    row.dispatchEvent(pointer("pointerdown", 275));
    row.dispatchEvent(pointer("pointermove", 290));
    vi.advanceTimersByTime(1000);
    expect(reorder.from).toBeNull();
    expect(dispatch("touchmove")).toBe(false);
  });

  it("ignore la souris, un appui sur un bouton, et une liste désactivée", () => {
    row.dispatchEvent(pointer("pointerdown", 275, { pointerType: "mouse" }));
    vi.advanceTimersByTime(350);
    expect(reorder.from).toBeNull();

    const onButton = pointer("pointerdown", 275);
    Object.defineProperty(onButton, "target", { value: { closest: () => ({}) } });
    row.dispatchEvent(onButton);
    vi.advanceTimersByTime(350);
    expect(reorder.from).toBeNull();

    action.update({ enabled: false, index: 1, reorder, items: () => rows as unknown as HTMLElement[] });
    hold();
    expect(reorder.from).toBeNull();
  });

  it("annulé par le navigateur : rien ne bouge", () => {
    hold();
    row.dispatchEvent(pointer("pointermove", 330));
    row.dispatchEvent(pointer("pointercancel", 330));
    expect(onMove).not.toHaveBeenCalled();
    expect([reorder.from, reorder.slot]).toEqual([null, null]);
    expect(row.classes.has("touch-dragging")).toBe(false);
  });

  it("désactivé en pleine saisie : la saisie est abandonnée", () => {
    hold();
    row.dispatchEvent(pointer("pointermove", 330));
    action.update({ enabled: false, index: 1, reorder, items: () => rows as unknown as HTMLElement[] });
    expect(reorder.from).toBeNull();
    row.dispatchEvent(pointer("pointerup", 330));
    expect(onMove).not.toHaveBeenCalled();
  });

  it("bloque le défilement et le menu de l'appui long pendant le geste seulement", () => {
    expect(dispatch("touchmove")).toBe(false);
    expect(dispatch("contextmenu")).toBe(false);
    row.dispatchEvent(pointer("pointerdown", 275));
    expect(dispatch("contextmenu")).toBe(true);
    vi.advanceTimersByTime(350);
    expect(dispatch("touchmove")).toBe(true);
    row.dispatchEvent(pointer("pointerup", 275));
    expect(dispatch("touchmove")).toBe(false);
  });

  it("avale le clic qui suit une saisie, dans la même tâche seulement", () => {
    hold();
    row.dispatchEvent(pointer("pointerup", 275));
    expect(dispatch("click")).toBe(true);
    vi.runAllTimers();
    expect(dispatch("click")).toBe(false);
    // Simple appui : le clic passe.
    row.dispatchEvent(pointer("pointerdown", 275));
    row.dispatchEvent(pointer("pointerup", 275));
    expect(dispatch("click")).toBe(false);
  });

  it("fait défiler la page près d'un bord, et la ligne suit le contenu", () => {
    hold();
    row.dispatchEvent(pointer("pointermove", 300));
    nextFrame();
    expect(scroller.scrollTop).toBe(0);

    // Au bord bas (600) : vitesse maximale.
    row.dispatchEvent(pointer("pointermove", 600));
    nextFrame();
    expect(scroller.scrollTop).toBe(14);
    expect(row.vars.get("--drag-y")).toBe(`${600 - 275 + 14}px`);
    nextFrame();
    expect(scroller.scrollTop).toBe(28);

    // Retour au milieu : le défilement s'arrête.
    row.dispatchEvent(pointer("pointermove", 300));
    nextFrame();
    expect(scroller.scrollTop).toBe(28);

    // Au bord haut : le haut de la liste est visible (172 px), rien à remonter (#302).
    row.dispatchEvent(pointer("pointermove", 32));
    nextFrame();
    expect(scroller.scrollTop).toBe(28);

    row.dispatchEvent(pointer("pointerup", 32));
    expect(frames).toHaveLength(0);
    expect(onMove).toHaveBeenCalledWith(1, 0);
  });

  it("remonte tant que le haut de la liste est caché, à demi-vitesse à mi-zone", () => {
    scroller.scrollTop = 180;
    hold(275 - 180);
    row.dispatchEvent(pointer("pointermove", 32));
    nextFrame();
    expect(scroller.scrollTop).toBe(173);
    // Le haut de la liste (200 - 173 = 27 px) n'est plus qu'à 37 px de la zone : dernier cran.
    for (let i = 0; i < 10; i++) nextFrame();
    expect(scroller.scrollTop).toBe(136);
    expect(200 - scroller.scrollTop).toBe(64);
  });

  it("s'arrête au bout de la liste au lieu de descendre sans fin (#302)", () => {
    hold();
    row.dispatchEvent(pointer("pointermove", 600));
    for (let i = 0; i < 100; i++) nextFrame();
    // Bas de la liste (800) au bord de la zone de défilement (600 - 64).
    expect(scroller.scrollTop).toBe(800 - 536);
    // La ligne ne dépasse pas la dernière : son bas s'arrête à celui de la liste.
    const { top } = row.getBoundingClientRect();
    expect(top + ROW).toBe(800 - scroller.scrollTop);
    row.dispatchEvent(pointer("pointerup", 600));
    expect(onMove).toHaveBeenCalledWith(1, COUNT - 1);
  });

  describe("avec une poignée (#450)", () => {
    /** Appui dont la cible est (ou non) dans la poignée `.grip`. */
    function press(onHandle: boolean, y = 275) {
      const e = pointer("pointerdown", y);
      const target = { closest: (sel: string) => (onHandle && sel === ".grip" ? {} : null) };
      Object.defineProperty(e, "target", { value: target });
      row.dispatchEvent(e);
    }

    beforeEach(() => {
      action.update({ enabled: true, index: 1, reorder, items: () => rows as unknown as HTMLElement[], handle: ".grip" });
    });

    it("saisit la ligne dès l'appui sur la poignée, sans appui long", () => {
      press(true);
      expect(reorder.from).toBe(1);
      expect(vibrate).toHaveBeenCalledOnce();
      // La page ne défile pas, et l'appui long ne déclenche ni menu ni sélection.
      expect(dispatch("touchmove")).toBe(true);
      expect(dispatch("contextmenu")).toBe(true);
      row.dispatchEvent(pointer("pointermove", 330));
      row.dispatchEvent(pointer("pointerup", 330));
      expect(onMove).toHaveBeenCalledWith(1, 2);
    });

    it("ailleurs sur la ligne, l'appui long garde son menu", () => {
      press(false);
      vi.advanceTimersByTime(1000);
      expect(reorder.from).toBeNull();
      expect(dispatch("contextmenu")).toBe(false);
    });

    it("un simple appui sur la poignée ne déplace rien", () => {
      press(true);
      row.dispatchEvent(pointer("pointerup", 275));
      expect(onMove).not.toHaveBeenCalled();
      expect([reorder.from, reorder.slot]).toEqual([null, null]);
    });
  });

  it("garde la ligne dans une liste courte, sans faire défiler la page", () => {
    setRows(3);
    hold();
    // Bas de la liste à 350 : la ligne (origine 250) ne descend que de 50 px.
    row.dispatchEvent(pointer("pointermove", 590));
    nextFrame();
    expect(scroller.scrollTop).toBe(0);
    expect(row.vars.get("--drag-y")).toBe("50px");
    expect(reorder.slot).toBe(3);
    // Ni au-dessus de la première.
    row.dispatchEvent(pointer("pointermove", 10));
    nextFrame();
    expect(row.vars.get("--drag-y")).toBe("-50px");
    expect(scroller.scrollTop).toBe(0);
    row.dispatchEvent(pointer("pointerup", 10));
    expect(onMove).toHaveBeenCalledWith(1, 0);
  });
});
