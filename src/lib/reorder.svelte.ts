// Réorganisation d'une liste par glisser-déposer HTML5 (file manuelle #80, playlist #201).
// L'appelant pose les gestionnaires sur chaque ligne et lit `marker` pour le repère
// d'insertion ; `onMove(from, to)` reçoit la position finale de l'entrée déplacée.

export class Reorder {
  /** Ligne en cours de glisser. */
  from = $state<number | null>(null);
  /** Emplacement d'insertion : avant la ligne `slot` (= longueur : à la fin). */
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
    const { from, slot } = this;
    if (from !== null && slot !== null) {
      // L'entrée retirée décale d'un cran les emplacements qui la suivent.
      const to = slot > from ? slot - 1 : slot;
      if (to !== from) this.onMove(from, to);
    }
    this.end();
  }

  /** À poser sur le conteneur : le repère disparaît quand on en sort. */
  dragleave(e: DragEvent, container: HTMLElement | undefined) {
    if (!container?.contains(e.relatedTarget as Node | null)) this.slot = null;
  }

  end() {
    this.from = null;
    this.slot = null;
  }

  /** Repère d'insertion avant `slot`, sauf là où le dépôt ne changerait rien. */
  marker(slot: number): boolean {
    const from = this.from;
    return from !== null && this.slot === slot && slot !== from && slot !== from + 1;
  }
}

/** Alt+↑ / Alt+↓ sur une ligne : position visée, ou `null` (autre touche, bord de liste). */
export function keyboardMove(e: KeyboardEvent, i: number, length: number): number | null {
  if (!e.altKey || (e.key !== "ArrowUp" && e.key !== "ArrowDown")) return null;
  e.preventDefault();
  const to = e.key === "ArrowUp" ? i - 1 : i + 1;
  return to < 0 || to >= length ? null : to;
}
