// Swiping a track row to the right (Android, #174): adds the track to the queue.
// Deliberately not very sensitive, so that scrolling never triggers anything:
// - `touch-action: pan-y` (the row's CSS): vertical scrolling stays with the browser, which
//   cancels the gesture (`pointercancel`) as soon as it takes over;
// - direction decided after LOCK px, gesture abandoned if it is not clearly horizontal;
// - triggered only beyond a share of the row's width, on release.
// The movement goes through a CSS variable and classes set here, without Svelte state: nothing
// is recomputed on every frame of the finger.

/** Distance before deciding the direction (px). */
const LOCK = 12;
/** The gesture must be at least DOMINANCE times more horizontal than vertical. */
const DOMINANCE = 2;
/** Left edge reserved for Android's Back with gesture navigation (px). */
const EDGE = 24;
/** Trigger threshold: share of the row's width, with a minimum in px. */
const RATIO = 0.35;
const MIN_DISTANCE = 96;
/** Duration of the row's return to its place (ms), like the CSS transition. */
const RELEASE_MS = 180;

export interface SwipeOptions {
  enabled: boolean;
  onSwipe: () => void;
}

export function swipeRight(node: HTMLElement, options: SwipeOptions) {
  let opts = options;
  let pointer: number | null = null;
  let phase: "idle" | "pending" | "swiping" = "idle";
  let x0 = 0;
  let y0 = 0;
  let dx = 0;
  let threshold = MIN_DISTANCE;
  /** A swipe has just ended: the click that follows is not a press on the row. */
  let swallowClick = false;
  let releaseTimer: ReturnType<typeof setTimeout> | undefined;

  function stop() {
    pointer = null;
    phase = "idle";
  }

  function release() {
    node.classList.remove("swipe-armed");
    node.classList.add("swipe-release");
    node.style.setProperty("--swipe-x", "0px");
    node.style.setProperty("--swipe-fade", "0");
    clearTimeout(releaseTimer);
    releaseTimer = setTimeout(() => {
      node.classList.remove("swiping", "swipe-release");
      node.style.removeProperty("--swipe-x");
      node.style.removeProperty("--swipe-fade");
    }, RELEASE_MS);
  }

  function onDown(e: PointerEvent) {
    if (!opts.enabled || !e.isPrimary || e.button !== 0) return;
    if (e.clientX < EDGE) return;
    pointer = e.pointerId;
    phase = "pending";
    x0 = e.clientX;
    y0 = e.clientY;
    dx = 0;
    threshold = Math.max(MIN_DISTANCE, node.getBoundingClientRect().width * RATIO);
  }

  function onMove(e: PointerEvent) {
    if (e.pointerId !== pointer || phase === "idle") return;
    const mx = e.clientX - x0;
    const my = e.clientY - y0;
    if (phase === "pending") {
      if (Math.hypot(mx, my) < LOCK) return;
      if (mx <= 0 || mx < DOMINANCE * Math.abs(my)) return stop();
      phase = "swiping";
      // Keeps the finger even if it leaves the row; fails if the pointer is no longer active.
      try {
        node.setPointerCapture(e.pointerId);
      } catch {
        /* without capture, the gesture continues as long as the finger stays on the row */
      }
      clearTimeout(releaseTimer);
      node.classList.remove("swipe-release");
      node.classList.add("swiping");
    }
    dx = Math.max(0, mx);
    node.style.setProperty("--swipe-x", `${dx}px`);
    node.style.setProperty("--swipe-fade", String(Math.min(1, dx / 48)));
    node.classList.toggle("swipe-armed", dx >= threshold);
  }

  function onUp(e: PointerEvent) {
    if (e.pointerId !== pointer) return;
    const swiped = phase === "swiping";
    const fire = swiped && dx >= threshold;
    stop();
    if (!swiped) return;
    release();
    // The possible click arrives right after `pointerup`, in the same task.
    swallowClick = true;
    setTimeout(() => (swallowClick = false), 0);
    if (fire) opts.onSwipe();
  }

  function onCancel(e: PointerEvent) {
    if (e.pointerId !== pointer) return;
    const swiped = phase === "swiping";
    stop();
    if (swiped) release();
  }

  // Capture phase: before the handlers of the row and its buttons.
  function onClick(e: MouseEvent) {
    if (!swallowClick) return;
    swallowClick = false;
    e.preventDefault();
    e.stopPropagation();
  }

  function onContextmenu(e: MouseEvent) {
    if (phase !== "swiping") return;
    e.preventDefault();
    e.stopPropagation();
  }

  node.addEventListener("pointerdown", onDown);
  node.addEventListener("pointermove", onMove);
  node.addEventListener("pointerup", onUp);
  node.addEventListener("pointercancel", onCancel);
  node.addEventListener("click", onClick, true);
  node.addEventListener("contextmenu", onContextmenu, true);

  return {
    update(next: SwipeOptions) {
      opts = next;
      if (!opts.enabled && phase !== "idle") {
        const swiped = phase === "swiping";
        stop();
        if (swiped) release();
      }
    },
    destroy() {
      clearTimeout(releaseTimer);
      node.removeEventListener("pointerdown", onDown);
      node.removeEventListener("pointermove", onMove);
      node.removeEventListener("pointerup", onUp);
      node.removeEventListener("pointercancel", onCancel);
      node.removeEventListener("click", onClick, true);
      node.removeEventListener("contextmenu", onContextmenu, true);
    },
  };
}
