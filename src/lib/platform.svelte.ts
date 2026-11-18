// Plateforme de l'app, lue une fois au démarrage (décidée côté Rust). Sur Android, l'UI
// passe en mise en page tactile et masque les réglages du bureau ; `data-platform` sur
// `<html>` sert aux styles.

import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { commands, type Platform } from "./ipc";

export const platform = $state<{ name: Platform; mobile: boolean }>({
  name: "desktop",
  mobile: false,
});

/** Repli sans IPC : la WebView d'Android annonce toujours « Android » dans son user-agent. */
export function guessPlatform(userAgent: string): Platform {
  return /\bAndroid\b/.test(userAgent) ? "android" : "desktop";
}

/** Commande `platform`, réessayée : au démarrage à froid sur Android, le premier appel IPC,
 *  fait avant le montage, échoue parfois (#472). Après les essais, on garde le repli du
 *  user-agent plutôt que de poser la mise en page du bureau sur un téléphone. */
async function askPlatform(fallback: Platform): Promise<Platform> {
  const delays = [0, 50, 200];
  let lastError: unknown;
  for (const delay of delays) {
    if (delay) await new Promise((resolve) => setTimeout(resolve, delay));
    try {
      return await commands.platform();
    } catch (e) {
      lastError = e;
    }
  }
  console.warn("platform() a échoué, repli sur le user-agent :", lastError);
  return fallback;
}

export async function initPlatform() {
  platform.name = await askPlatform(guessPlatform(navigator.userAgent));
  platform.mobile = platform.name === "android";
  document.documentElement.dataset.platform = platform.name;
}

/** Mini-lecteur du bureau (#384) : la fenêtre `mini` charge la même page. L'étiquette est
 *  lue dans les métadonnées de la WebView, sans aller-retour. Hors Tauri : `?mini`. */
export function isMiniWindow(): boolean {
  if ("__TAURI_INTERNALS__" in window) return getCurrentWebviewWindow().label === "mini";
  return import.meta.env.DEV && new URLSearchParams(location.search).has("mini");
}
