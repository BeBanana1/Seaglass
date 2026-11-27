import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Reorder } from "./reorder.svelte";
import { touchReorder } from "./touch-reorder";

// Fake list, without a simulated DOM: a scrolling container (top 0, bottom 600) and twelve rows
// of 50 px placed from 200 px in the content (up to 800, lower than the screen).
// The grabbed row is the second one.
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

  /** Position on screen: follows the scrolling and, for the grabbed row, its `--drag-y`. */
  getBoundingClientRect() {
    const shift = parseFloat(this.vars.get("--drag-y") ?? "0");
    const top = TOP + this.index * ROW - this.parentElement.scrollTop + shift;
    return { x: 0, y: top, top, width: 300, height: ROW };
  }
}

let frames: FrameRequestCallback[] = [];
const vibrate = vi.fn();

/** Plays the pending animation frames (automatic scrolling). */
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

/** Replaces the list with `count` rows (the grab stays on the second). */
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

/** Long press on the grabbed row (in the middle, y = 275). */
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
  vi.stubGlobal("getComputedStyle", (el: unknown) => ({
    overflowY: el instanceof FakeScroller ? "auto" : "visible",
  }));
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
  it("grabs the row after a long press, with a vibration", () => {
    row.dispatchEvent(pointer("pointerdown", 275));
    vi.advanceTimersByTime(349);
    expect(reorder.from).toBeNull();
    vi.advanceTimersByTime(1);
    expect(reorder.from).toBe(1);
    expect(row.classes.has("touch-dragging")).toBe(true);
    expect(vibrate).toHaveBeenCalledOnce();
    // Without moving, the row stays in place: the drop changes nothing.
    expect(reorder.slot).toBe(2);
    row.dispatchEvent(pointer("pointerup", 275));
    expect(onMove).not.toHaveBeenCalled();
  });

  it("moves down: the row follows the finger, located from its original place", () => {
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

  it("moves up", () => {
    hold();
    // Above its own original middle: slot right before it (no effect).
    row.dispatchEvent(pointer("pointermove", 260));
    expect(reorder.slot).toBe(1);
    row.dispatchEvent(pointer("pointermove", 210));
    expect(reorder.slot).toBe(0);
    row.dispatchEvent(pointer("pointerup", 210));
    expect(onMove).toHaveBeenCalledWith(1, 0);
  });

  it("tolerates a slight movement during the press", () => {
    row.dispatchEvent(pointer("pointerdown", 275));
    row.dispatchEvent(pointer("pointermove", 282));
    vi.advanceTimersByTime(350);
    expect(reorder.from).toBe(1);
  });

  it("hands control back to scrolling if the finger moves before the grab", () => {
    row.dispatchEvent(pointer("pointerdown", 275));
    row.dispatchEvent(pointer("pointermove", 290));
    vi.advanceTimersByTime(1000);
    expect(reorder.from).toBeNull();
    expect(dispatch("touchmove")).toBe(false);
  });

  it("ignores the mouse, a press on a button, and a disabled list", () => {
    row.dispatchEvent(pointer("pointerdown", 275, { pointerType: "mouse" }));
    vi.advanceTimersByTime(350);
    expect(reorder.from).toBeNull();

    const onButton = pointer("pointerdown", 275);
    Object.defineProperty(onButton, "target", { value: { closest: () => ({}) } });
    row.dispatchEvent(onButton);
    vi.advanceTimersByTime(350);
    expect(reorder.from).toBeNull();

    action.update({
      enabled: false,
      index: 1,
      reorder,
      items: () => rows as unknown as HTMLElement[],
    });
    hold();
    expect(reorder.from).toBeNull();
  });

  it("cancelled by the browser: nothing moves", () => {
    hold();
    row.dispatchEvent(pointer("pointermove", 330));
    row.dispatchEvent(pointer("pointercancel", 330));
    expect(onMove).not.toHaveBeenCalled();
    expect([reorder.from, reorder.slot]).toEqual([null, null]);
    expect(row.classes.has("touch-dragging")).toBe(false);
  });

  it("disabled in the middle of a grab: the grab is abandoned", () => {
    hold();
    row.dispatchEvent(pointer("pointermove", 330));
    action.update({
      enabled: false,
      index: 1,
      reorder,
      items: () => rows as unknown as HTMLElement[],
    });
    expect(reorder.from).toBeNull();
    row.dispatchEvent(pointer("pointerup", 330));
    expect(onMove).not.toHaveBeenCalled();
  });

  it("blocks scrolling and the long-press menu during the gesture only", () => {
    expect(dispatch("touchmove")).toBe(false);
    expect(dispatch("contextmenu")).toBe(false);
    row.dispatchEvent(pointer("pointerdown", 275));
    expect(dispatch("contextmenu")).toBe(true);
    vi.advanceTimersByTime(350);
    expect(dispatch("touchmove")).toBe(true);
    row.dispatchEvent(pointer("pointerup", 275));
    expect(dispatch("touchmove")).toBe(false);
  });

  it("swallows the click that follows a grab, in the same task only", () => {
    hold();
    row.dispatchEvent(pointer("pointerup", 275));
    expect(dispatch("click")).toBe(true);
    vi.runAllTimers();
    expect(dispatch("click")).toBe(false);
    // Simple press: the click goes through.
    row.dispatchEvent(pointer("pointerdown", 275));
    row.dispatchEvent(pointer("pointerup", 275));
    expect(dispatch("click")).toBe(false);
  });

  it("scrolls the page near an edge, and the row follows the content", () => {
    hold();
    row.dispatchEvent(pointer("pointermove", 300));
    nextFrame();
    expect(scroller.scrollTop).toBe(0);

    // At the bottom edge (600): maximum speed.
    row.dispatchEvent(pointer("pointermove", 600));
    nextFrame();
    expect(scroller.scrollTop).toBe(14);
    expect(row.vars.get("--drag-y")).toBe(`${600 - 275 + 14}px`);
    nextFrame();
    expect(scroller.scrollTop).toBe(28);

    // Back to the middle: scrolling stops.
    row.dispatchEvent(pointer("pointermove", 300));
    nextFrame();
    expect(scroller.scrollTop).toBe(28);

    // At the top edge: the top of the list is visible (172 px), nothing to scroll up (#302).
    row.dispatchEvent(pointer("pointermove", 32));
    nextFrame();
    expect(scroller.scrollTop).toBe(28);

    row.dispatchEvent(pointer("pointerup", 32));
    expect(frames).toHaveLength(0);
    expect(onMove).toHaveBeenCalledWith(1, 0);
  });

  it("scrolls up while the top of the list is hidden, at half speed mid-zone", () => {
    scroller.scrollTop = 180;
    hold(275 - 180);
    row.dispatchEvent(pointer("pointermove", 32));
    nextFrame();
    expect(scroller.scrollTop).toBe(173);
    // The top of the list (200 - 173 = 27 px) is only 37 px from the zone: last notch.
    for (let i = 0; i < 10; i++) nextFrame();
    expect(scroller.scrollTop).toBe(136);
    expect(200 - scroller.scrollTop).toBe(64);
  });

  it("stops at the end of the list instead of going down endlessly (#302)", () => {
    hold();
    row.dispatchEvent(pointer("pointermove", 600));
    for (let i = 0; i < 100; i++) nextFrame();
    // Bottom of the list (800) at the edge of the scroll zone (600 - 64).
    expect(scroller.scrollTop).toBe(800 - 536);
    // The row does not go past the last one: its bottom stops at the list's.
    const { top } = row.getBoundingClientRect();
    expect(top + ROW).toBe(800 - scroller.scrollTop);
    row.dispatchEvent(pointer("pointerup", 600));
    expect(onMove).toHaveBeenCalledWith(1, COUNT - 1);
  });

  describe("with a handle (#450)", () => {
    /** Press whose target is (or is not) in the `.grip` handle. */
    function press(onHandle: boolean, y = 275) {
      const e = pointer("pointerdown", y);
      const target = { closest: (sel: string) => (onHandle && sel === ".grip" ? {} : null) };
      Object.defineProperty(e, "target", { value: target });
      row.dispatchEvent(e);
    }

    beforeEach(() => {
      action.update({
        enabled: true,
        index: 1,
        reorder,
        items: () => rows as unknown as HTMLElement[],
        handle: ".grip",
      });
    });

    it("grabs the row as soon as the handle is pressed, without a long press", () => {
      press(true);
      expect(reorder.from).toBe(1);
      expect(vibrate).toHaveBeenCalledOnce();
      // The page does not scroll, and the long press triggers neither menu nor selection.
      expect(dispatch("touchmove")).toBe(true);
      expect(dispatch("contextmenu")).toBe(true);
      row.dispatchEvent(pointer("pointermove", 330));
      row.dispatchEvent(pointer("pointerup", 330));
      expect(onMove).toHaveBeenCalledWith(1, 2);
    });

    it("elsewhere on the row, the long press keeps its menu", () => {
      press(false);
      vi.advanceTimersByTime(1000);
      expect(reorder.from).toBeNull();
      expect(dispatch("contextmenu")).toBe(false);
    });

    it("a simple press on the handle moves nothing", () => {
      press(true);
      row.dispatchEvent(pointer("pointerup", 275));
      expect(onMove).not.toHaveBeenCalled();
      expect([reorder.from, reorder.slot]).toEqual([null, null]);
    });
  });

  it("keeps the row in a short list, without scrolling the page", () => {
    setRows(3);
    hold();
    // Bottom of the list at 350: the row (origin 250) only goes down by 50 px.
    row.dispatchEvent(pointer("pointermove", 590));
    nextFrame();
    expect(scroller.scrollTop).toBe(0);
    expect(row.vars.get("--drag-y")).toBe("50px");
    expect(reorder.slot).toBe(3);
    // Nor above the first one.
    row.dispatchEvent(pointer("pointermove", 10));
    nextFrame();
    expect(row.vars.get("--drag-y")).toBe("-50px");
    expect(scroller.scrollTop).toBe(0);
    row.dispatchEvent(pointer("pointerup", 10));
    expect(onMove).toHaveBeenCalledWith(1, 0);
  });
});
