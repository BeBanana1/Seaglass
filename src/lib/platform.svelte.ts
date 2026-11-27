// App platform, read once at startup (decided on the Rust side). On Android, the UI
// switches to a touch layout and hides the desktop settings; `data-platform` on
// `<html>` is used by the styles.

import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { commands, type Platform } from "./ipc";

export const platform = $state<{ name: Platform; mobile: boolean }>({
  name: "desktop",
  mobile: false,
});

/** Fallback without IPC: Android's WebView always announces "Android" in its user-agent. */
export function guessPlatform(userAgent: string): Platform {
  return /\bAndroid\b/.test(userAgent) ? "android" : "desktop";
}

/** `platform` command, retried: on a cold start on Android, the first IPC call,
 *  made before mounting, sometimes fails (#472). After the attempts, we keep the
 *  user-agent fallback rather than putting the desktop layout on a phone. */
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
  console.warn("platform() failed, falling back to the user-agent:", lastError);
  return fallback;
}

export async function initPlatform() {
  platform.name = await askPlatform(guessPlatform(navigator.userAgent));
  platform.mobile = platform.name === "android";
  document.documentElement.dataset.platform = platform.name;
}

/** Desktop mini-player (#384): the `mini` window loads the same page. The label is
 *  read from the WebView's metadata, without a round trip. Outside Tauri: `?mini`. */
export function isMiniWindow(): boolean {
  if ("__TAURI_INTERNALS__" in window) return getCurrentWebviewWindow().label === "mini";
  return import.meta.env.DEV && new URLSearchParams(location.search).has("mini");
}
