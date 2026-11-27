// Reordering a list by HTML5 drag and drop (manual queue #80, playlist #201).
// The caller puts the handlers on each row and reads `marker` for the insertion
// indicator; `onMove(from, to)` receives the final position of the moved entry.

export class Reorder {
  /** Row being dragged. */
  from = $state<number | null>(null);
  /** Insertion slot: before row `slot` (= length: at the end). */
  slot = $state<number | null>(null);

  constructor(private readonly onMove: (from: number, to: number) => void) {}

  dragstart(e: DragEvent, i: number) {
    this.from = i;
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData("text/plain", "");
    }
  }

  dragover(e: DragEvent, i: number) {
    if (this.from === null) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    this.slot = e.clientY < rect.top + rect.height / 2 ? i : i + 1;
  }

  drop(e: DragEvent) {
    e.preventDefault();
    this.commit();
  }

  /** Applies the current move (HTML5 drop, or the end of a finger drag). */
  commit() {
    const { from, slot } = this;
    if (from !== null && slot !== null) {
      // The removed entry shifts the slots that follow it by one.
      const to = slot > from ? slot - 1 : slot;
      if (to !== from) this.onMove(from, to);
    }
    this.end();
  }

  /** To put on the container: the indicator disappears when leaving it. */
  dragleave(e: DragEvent, container: HTMLElement | undefined) {
    if (!container?.contains(e.relatedTarget as Node | null)) this.slot = null;
  }

  end() {
    this.from = null;
    this.slot = null;
  }

  /** Insertion indicator before `slot`, except where the drop would change nothing. */
  marker(slot: number): boolean {
    const from = this.from;
    return from !== null && this.slot === slot && slot !== from && slot !== from + 1;
  }
}

/** Alt+↑ / Alt+↓ on a row: target position, or `null` (other key, edge of the list). */
export function keyboardMove(e: KeyboardEvent, i: number, length: number): number | null {
  if (!e.altKey || (e.key !== "ArrowUp" && e.key !== "ArrowDown")) return null;
  e.preventDefault();
  const to = e.key === "ArrowUp" ? i - 1 : i + 1;
  return to < 0 || to >= length ? null : to;
}
