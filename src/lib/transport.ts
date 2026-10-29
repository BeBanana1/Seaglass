// Accès au backend. Dans Tauri : commandes et événements réels.
// En développement dans un navigateur ordinaire (sans Tauri), bascule sur `./mock` pour
// prévisualiser l'interface ; cette branche disparaît du build de production.

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event";

const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (import.meta.env.DEV && !inTauri) {
    return (await import("./mock")).invoke<T>(cmd, args);
  }
  return tauriInvoke<T>(cmd, args);
}

export async function listen<T>(
  event: string,
  cb: (e: { payload: T }) => void,
): Promise<UnlistenFn> {
  if (import.meta.env.DEV && !inTauri) {
    return (await import("./mock")).listen<T>(event, cb);
  }
  return tauriListen<T>(event, cb);
}

export type { UnlistenFn };
