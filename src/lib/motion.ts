// Interface animations (#351): pure logic, no DOM.

import type { Motion } from "./ipc";

/** Reduced animations: requested by the setting, or by the system if it follows it. */
export function motionReduced(motion: Motion, system: boolean): boolean {
  return motion === "reduced" || (motion === "system" && system);
}
