// Finger drag to reorder a list (Android, #290): HTML5 drag does not work
// with touch in the WebView. A long press (HOLD ms without moving) grabs the row, which
// then follows the finger; the insertion indicator and the final move go through the same
// `Reorder` as the mouse drag.
// - Before the grab, a move of more than SLOP px hands control back to scrolling.
// - After the grab, `touchmove` is cancelled (non-passive listener): the page no longer scrolls
//   under the finger; it scrolls by itself when the finger nears an edge of the container, but
//   only while there is list left to show on that side (#302).
// - The grabbed row stays in the list: otherwise its `transform` enlarged the scrollable
//   area and the automatic scrolling went down endlessly below the list (#302).
// - A press that starts on a button is not a grab (Move up / Move down, Hide).
// - With a handle (`handle`, queue #450), only a press on it grabs the row,
//   and immediately: the long press on the rest of the row keeps its menu. The handle carries
//   `touch-action: none`, so the page does not scroll under it.
// The row follows the finger through a CSS variable and classes set here, without Svelte state.

import type { Reorder } from "./reorder.svelte";

/** Duration of the press before the grab (ms). */
const HOLD = 350;
/** Tolerance of the finger's movement during the press (px). */
const SLOP = 10;
/** Zone near the container's edges where the page scrolls by itself (px). */
const EDGE = 64;
/** Maximum speed of the automatic scrolling (px per frame). */
const SPEED = 14;

export interface TouchReorderOptions {
  enabled: boolean;
  /** Position of the row in the reorderable list. */
  index: number;
  reorder: Reorder;
  /** Rows of the list, in order, to locate the finger. */
  items: () => HTMLElement[];
  /** Selector of the handle: the press must start there, and grabs without waiting. */
  handle?: string;
}

function scrollParent(node: HTMLElement): HTMLElement {
  for (let el = node.parentElement; el; el = el.parentElement) {
    const { overflowY } = getComputedStyle(el);
    if ((overflowY === "auto" || overflowY === "scroll") && el.scrollHeight > el.clientHeight)
      return el;
  }
  return (document.scrollingElement as HTMLElement) ?? document.documentElement;
}

export function touchReorder(node: HTMLElement, options: TouchReorderOptions) {
  let opts = options;
  let pointer: number | null = null;
  let phase: "idle" | "pending" | "dragging" = "idle";
  let timer: ReturnType<typeof setTimeout> | undefined;
  let x0 = 0;
  let y0 = 0;
  let y = 0;
  let scroller: HTMLElement | null = null;
  let scroll0 = 0;
  let frame = 0;
  /** Offset applied to the grabbed row (`--drag-y`), in px. */
  let shift = 0;
  /** A grab has just ended: the click that follows is not a press on the row. */
  let swallowClick = false;

  /** Top and bottom of the list on screen, the grabbed row counted at its original place. */
  function bounds(items: HTMLElement[]): { top: number; bottom: number } | null {
    if (items.length === 0) return null;
    const rect = (el: HTMLElement) => (el === node ? originRect() : el.getBoundingClientRect());
    const first = rect(items[0]);
    const last = rect(items[items.length - 1]);
    return { top: first.top, bottom: last.top + last.height };
  }

  function place() {
    if (!scroller) return;
    const items = opts.items();
    const origin = originRect();
    let next = y - y0 + scroller.scrollTop - scroll0;
    const list = bounds(items);
    if (list) {
      next = Math.min(next, list.bottom - origin.top - origin.height);
      next = Math.max(next, list.top - origin.top);
    }
    shift = next;
    node.style.setProperty("--drag-y", `${shift}px`);
    // Slot: before the first row whose middle is under the finger.
    let slot = items.length;
    for (let i = 0; i < items.length; i++) {
      // The grabbed row is moved by `transform`: it is located at its original place.
      const rect = items[i] === node ? originRect() : items[i].getBoundingClientRect();
      if (y < rect.top + rect.height / 2) {
        slot = i;
        break;
      }
    }
    opts.reorder.slot = slot;
  }

  function originRect(): DOMRect {
    const r = node.getBoundingClientRect();
    return new DOMRect(r.x, r.y - shift, r.width, r.height);
  }

  function autoScroll() {
    frame = 0;
    if (phase !== "dragging" || !scroller) return;
    const box =
      scroller === document.scrollingElement
        ? { top: 0, bottom: window.innerHeight }
        : scroller.getBoundingClientRect();
    let step = 0;
    if (y < box.top + EDGE) step = -SPEED * Math.min(1, (box.top + EDGE - y) / EDGE);
    else if (y > box.bottom - EDGE) step = SPEED * Math.min(1, (y - (box.bottom - EDGE)) / EDGE);
    // Not beyond the list: we stop when its end has left the edge zone.
    const list = bounds(opts.items());
    if (list && step > 0) step = Math.min(step, Math.max(0, list.bottom - (box.bottom - EDGE)));
    if (list && step < 0) step = Math.max(step, Math.min(0, list.top - (box.top + EDGE)));
    if (step !== 0) {
      const before = scroller.scrollTop;
      scroller.scrollTop += step;
      if (scroller.scrollTop !== before) place();
    }
    frame = requestAnimationFrame(autoScroll);
  }

  function grab() {
    phase = "dragging";
    shift = 0;
    scroller = scrollParent(node);
    scroll0 = scroller.scrollTop;
    node.classList.add("touch-dragging");
    opts.reorder.from = opts.index;
    navigator.vibrate?.(15);
    place();
    frame = requestAnimationFrame(autoScroll);
  }

  function finish(commit: boolean) {
    clearTimeout(timer);
    if (phase === "dragging") {
      if (commit) opts.reorder.commit();
      else opts.reorder.end();
      node.classList.remove("touch-dragging");
      node.style.removeProperty("--drag-y");
      shift = 0;
      swallowClick = true;
      setTimeout(() => (swallowClick = false), 0);
    }
    cancelAnimationFrame(frame);
    frame = 0;
    pointer = null;
    phase = "idle";
    scroller = null;
  }

  function onDown(e: PointerEvent) {
    if (!opts.enabled || !e.isPrimary || e.pointerType === "mouse") return;
    const target = e.target as Element;
    if (opts.handle ? !target.closest(opts.handle) : target.closest("button, a, input")) return;
    pointer = e.pointerId;
    phase = "pending";
    x0 = e.clientX;
    y0 = y = e.clientY;
    clearTimeout(timer);
    if (opts.handle) grab();
    else timer = setTimeout(grab, HOLD);
  }

  function onMove(e: PointerEvent) {
    if (e.pointerId !== pointer) return;
    if (phase === "pending") {
      if (Math.hypot(e.clientX - x0, e.clientY - y0) > SLOP) finish(false);
      return;
    }
    y = e.clientY;
    place();
  }

  function onUp(e: PointerEvent) {
    if (e.pointerId === pointer) finish(true);
  }

  function onCancel(e: PointerEvent) {
    if (e.pointerId === pointer) finish(false);
  }

  /** After the grab, the finger moves the row and not the page. */
  function onTouchMove(e: TouchEvent) {
    if (phase === "dragging" && e.cancelable) e.preventDefault();
  }

  /** The long press would otherwise open the menu or the WebView's text selection. */
  function onContextMenu(e: Event) {
    if (phase !== "idle") e.preventDefault();
  }

  function onClick(e: MouseEvent) {
    if (!swallowClick) return;
    e.preventDefault();
    e.stopPropagation();
  }

  node.addEventListener("pointerdown", onDown);
  node.addEventListener("pointermove", onMove);
  node.addEventListener("pointerup", onUp);
  node.addEventListener("pointercancel", onCancel);
  node.addEventListener("touchmove", onTouchMove, { passive: false });
  node.addEventListener("contextmenu", onContextMenu);
  node.addEventListener("click", onClick, true);

  return {
    update(next: TouchReorderOptions) {
      opts = next;
      if (!opts.enabled && phase !== "idle") finish(false);
    },
    destroy() {
      finish(false);
      node.removeEventListener("pointerdown", onDown);
      node.removeEventListener("pointermove", onMove);
      node.removeEventListener("pointerup", onUp);
      node.removeEventListener("pointercancel", onCancel);
      node.removeEventListener("touchmove", onTouchMove);
      node.removeEventListener("contextmenu", onContextMenu);
      node.removeEventListener("click", onClick, true);
    },
  };
}
