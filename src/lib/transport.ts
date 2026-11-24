// Access to the backend. In Tauri: real commands and events.
// In development in a plain browser (without Tauri), and in the website demo build (#28),
// switches to `./mock` to preview the interface; this branch is dropped from the app's
// production build (`import.meta.env.DEV` and `__DEMO__` are both false there).

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event";

const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if ((import.meta.env.DEV || __DEMO__) && !inTauri) {
    return (await import("./mock")).invoke<T>(cmd, args);
  }
  return tauriInvoke<T>(cmd, args);
}

export async function listen<T>(
  event: string,
  cb: (e: { payload: T }) => void,
): Promise<UnlistenFn> {
  if ((import.meta.env.DEV || __DEMO__) && !inTauri) {
    return (await import("./mock")).listen<T>(event, cb);
  }
  return tauriListen<T>(event, cb);
}

export type { UnlistenFn };
