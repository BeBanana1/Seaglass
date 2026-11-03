// Plein écran de la fenêtre (#131), miroir de l'état tenu par le backend. Bureau seulement :
// sur Android, l'app est déjà en plein écran et les commandes n'existent pas.

import { commands, events } from "../ipc";
import { platform } from "../platform.svelte";
import { pushToast } from "./toasts.svelte";

export const appWindow = $state({ fullscreen: false });

export async function initWindow() {
  if (platform.mobile) return;
  await events.onFullscreen((on) => (appWindow.fullscreen = on));
  try {
    appWindow.fullscreen = await commands.fullscreen();
  } catch {
    // État par défaut : fenêtre normale.
  }
}

export async function setFullscreen(on: boolean) {
  if (platform.mobile) return;
  try {
    appWindow.fullscreen = await commands.setFullscreen(on);
  } catch (e) {
    pushToast({ level: "error", message: String(e) });
  }
}

export function toggleFullscreen() {
  return setFullscreen(!appWindow.fullscreen);
}
