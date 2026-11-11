// Animations de l'interface (#351) : logique pure, sans DOM.

import type { Motion } from "./ipc";

/** Animations réduites : demandé par le réglage, ou par le système s'il le suit. */
export function motionReduced(motion: Motion, system: boolean): boolean {
  return motion === "reduced" || (motion === "system" && system);
}
