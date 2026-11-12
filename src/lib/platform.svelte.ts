// Plateforme de l'app, lue une fois au démarrage (décidée côté Rust). Sur Android, l'UI
// passe en mise en page tactile et masque les réglages du bureau ; `data-platform` sur
// `<html>` sert aux styles.

import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { commands, type Platform } from "./ipc";

export const platform = $state<{ name: Platform; mobile: boolean }>({ name: "desktop", mobile: false });

export async function initPlatform() {
  try {
    platform.name = await commands.platform();
  } catch {
    platform.name = "desktop";
  }
  platform.mobile = platform.name === "android";
  document.documentElement.dataset.platform = platform.name;
}

/** Mini-lecteur du bureau (#384) : la fenêtre `mini` charge la même page. L'étiquette est
 *  lue dans les métadonnées de la WebView, sans aller-retour. Hors Tauri : `?mini`. */
export function isMiniWindow(): boolean {
  if ("__TAURI_INTERNALS__" in window) return getCurrentWebviewWindow().label === "mini";
  return import.meta.env.DEV && new URLSearchParams(location.search).has("mini");
}
