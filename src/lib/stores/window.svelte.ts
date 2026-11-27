// Window full screen (#131), mirror of the state held by the backend. Desktop only: on
// Android, the app is already full screen and the commands do not exist.

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
    // Default state: normal window.
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
