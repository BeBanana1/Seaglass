import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { swipeRight } from "./swipe";

// Fake row: just what `swipeRight` reads and writes, without a simulated DOM.
class FakeRow extends EventTarget {
  readonly classes = new Set<string>();
  readonly vars = new Map<string, string>();
  captured: number | null = null;

  classList = {
    add: (...names: string[]) => names.forEach((n) => this.classes.add(n)),
    remove: (...names: string[]) => names.forEach((n) => this.classes.delete(n)),
    toggle: (name: string, force: boolean) =>
      force ? this.classes.add(name) : this.classes.delete(name),
  };

  style = {
    setProperty: (name: string, value: string) => this.vars.set(name, value),
    removeProperty: (name: string) => this.vars.delete(name),
  };

  constructor(private readonly width = 400) {
    super();
  }

  getBoundingClientRect() {
    return { width: this.width };
  }

  setPointerCapture(id: number) {
    this.captured = id;
  }
}

function pointer(
  type: string,
  clientX: number,
  clientY = 100,
  extra: Record<string, unknown> = {},
) {
  return Object.assign(new Event(type, { cancelable: true }), {
    pointerId: 1,
    isPrimary: true,
    button: 0,
    clientX,
    clientY,
    ...extra,
  });
}

let row: FakeRow;
let onSwipe: ReturnType<typeof vi.fn<() => void>>;
let action: ReturnType<typeof swipeRight>;

/** Finger placed at x = 100, moved in steps, then lifted. */
function gesture(steps: [number, number][], end: "pointerup" | "pointercancel" = "pointerup") {
  row.dispatchEvent(pointer("pointerdown", 100));
  for (const [x, y] of steps) row.dispatchEvent(pointer("pointermove", x, y));
  row.dispatchEvent(pointer(end, steps.at(-1)?.[0] ?? 100));
}

/** Click that follows the gesture: `true` if it reaches the row. */
function click() {
  const e = new Event("click", { cancelable: true });
  row.dispatchEvent(e);
  return !e.defaultPrevented;
}

beforeEach(() => {
  vi.useFakeTimers();
  row = new FakeRow();
  onSwipe = vi.fn<() => void>();
  action = swipeRight(row as unknown as HTMLElement, { enabled: true, onSwipe });
});

afterEach(() => {
  action.destroy();
  vi.useRealTimers();
});

describe("swipeRight", () => {
  it("triggers beyond the threshold (35% of the width), on release only", () => {
    row.dispatchEvent(pointer("pointerdown", 100));
    row.dispatchEvent(pointer("pointermove", 120));
    row.dispatchEvent(pointer("pointermove", 260));
    expect(row.classes.has("swipe-armed")).toBe(true);
    expect(row.captured).toBe(1);
    expect(onSwipe).not.toHaveBeenCalled();
    row.dispatchEvent(pointer("pointerup", 260));
    expect(onSwipe).toHaveBeenCalledOnce();
  });

  it("does not trigger below the threshold, and brings the row back", () => {
    gesture([
      [120, 100],
      [230, 100],
    ]);
    expect(onSwipe).not.toHaveBeenCalled();
    expect(row.classes.has("swipe-release")).toBe(true);
    expect(row.vars.get("--swipe-x")).toBe("0px");
    vi.runAllTimers();
    expect(row.classes.size).toBe(0);
    expect(row.vars.size).toBe(0);
  });

  it("keeps a minimal threshold on a narrow row", () => {
    action.destroy();
    row = new FakeRow(100);
    action = swipeRight(row as unknown as HTMLElement, { enabled: true, onSwipe });
    gesture([
      [120, 100],
      [190, 100],
    ]);
    expect(onSwipe).not.toHaveBeenCalled();
    gesture([
      [120, 100],
      [200, 100],
    ]);
    expect(onSwipe).toHaveBeenCalledOnce();
  });

  it("abandons a gesture that is not horizontal enough or goes left", () => {
    gesture([
      [110, 110],
      [400, 110],
    ]);
    gesture([
      [80, 100],
      [400, 100],
    ]);
    expect(onSwipe).not.toHaveBeenCalled();
    expect(row.classes.has("swiping")).toBe(false);
  });

  it("waits a few pixels before deciding the direction", () => {
    row.dispatchEvent(pointer("pointerdown", 100));
    row.dispatchEvent(pointer("pointermove", 105, 104));
    expect(row.classes.has("swiping")).toBe(false);
    row.dispatchEvent(pointer("pointermove", 140, 104));
    expect(row.classes.has("swiping")).toBe(true);
  });

  it("leaves the left edge to Android's Back", () => {
    row.dispatchEvent(pointer("pointerdown", 10));
    row.dispatchEvent(pointer("pointermove", 300));
    row.dispatchEvent(pointer("pointerup", 300));
    expect(onSwipe).not.toHaveBeenCalled();
  });

  it("ignores another finger or another button", () => {
    row.dispatchEvent(pointer("pointerdown", 100, 100, { button: 2 }));
    row.dispatchEvent(pointer("pointermove", 300));
    row.dispatchEvent(pointer("pointerup", 300));
    row.dispatchEvent(pointer("pointerdown", 100));
    row.dispatchEvent(pointer("pointermove", 300, 100, { pointerId: 2 }));
    row.dispatchEvent(pointer("pointerup", 300, 100, { pointerId: 2 }));
    expect(onSwipe).not.toHaveBeenCalled();
  });

  it("cancelled by the browser (scrolling): nothing triggers", () => {
    gesture(
      [
        [120, 100],
        [300, 100],
      ],
      "pointercancel",
    );
    expect(onSwipe).not.toHaveBeenCalled();
    expect(row.classes.has("swipe-release")).toBe(true);
  });

  it("swallows the click that follows a swipe, not the others", () => {
    gesture([
      [120, 100],
      [150, 100],
    ]);
    expect(click()).toBe(false);
    expect(click()).toBe(true);
    // Simple press: the click goes through.
    gesture([]);
    expect(click()).toBe(true);
  });

  it("the swallowed click does not outlive the release task", () => {
    gesture([
      [120, 100],
      [150, 100],
    ]);
    vi.runAllTimers();
    expect(click()).toBe(true);
  });

  it("disabled: ignores the gesture, and brings back a row being dragged", () => {
    row.dispatchEvent(pointer("pointerdown", 100));
    row.dispatchEvent(pointer("pointermove", 200));
    action.update({ enabled: false, onSwipe });
    expect(row.classes.has("swipe-release")).toBe(true);
    row.dispatchEvent(pointer("pointerup", 300));
    expect(onSwipe).not.toHaveBeenCalled();
    gesture([
      [120, 100],
      [300, 100],
    ]);
    expect(onSwipe).not.toHaveBeenCalled();
  });
});
