// Plateforme de l'app, lue une fois au démarrage (décidée côté Rust). Sur Android, l'UI
// passe en mise en page tactile et masque les réglages du bureau ; `data-platform` sur
// `<html>` sert aux styles.

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
