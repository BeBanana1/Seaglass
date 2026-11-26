// Playback handoff with the linked computer (#414), phone side, and synced listening
// history (#430). The backend keeps the link (Keystore) and does the exchanges; this store
// only keeps what the UI shows.

import { commands, events, type LinkedComputer } from "../ipc";
import { t } from "../i18n.svelte";
import { platform } from "../platform.svelte";
import { pushToast } from "./toasts.svelte";

export const handoff = $state<{
  linked: LinkedComputer | null;
  loaded: boolean;
  busy: boolean;
  syncing: boolean;
}>({
  linked: null,
  loaded: false,
  busy: false,
  syncing: false,
});

/** Reads the linked computer once (Android only: the desktop has no button). */
export async function loadLinkedComputer() {
  if (!platform.mobile || handoff.loaded) return;
  try {
    handoff.linked = await commands.linkedComputer();
    handoff.loaded = true;
  } catch (e) {
    pushToast({ level: "error", message: String(e) });
  }
}

async function run(action: () => Promise<void>, done: string) {
  if (handoff.busy) return;
  handoff.busy = true;
  try {
    await action();
    pushToast({ level: "info", message: done });
  } catch (e) {
    pushToast({ level: "error", message: String(e) });
  } finally {
    handoff.busy = false;
  }
}

/** "Resume here": the computer's playback moves to the phone. */
export const pullHere = () => run(commands.handoffPull, t.handoff.pulled);

/** "Continue on the computer". */
export const pushToComputer = () => run(commands.handoffPush, t.handoff.pushed);

/** "Sync" in Settings: listening history exchanged right away (#430). */
export async function syncHistoryNow() {
  if (handoff.syncing) return;
  handoff.syncing = true;
  try {
    const at = await commands.syncHistory(true);
    if (handoff.linked) handoff.linked = { ...handoff.linked, syncedAt: at };
    pushToast({ level: "info", message: t.handoff.synced });
  } catch (e) {
    pushToast({ level: "error", message: String(e) });
  } finally {
    handoff.syncing = false;
  }
}

/**
 * History views (home, statistics, recap): `reload` when plays arrive from the other
 * device. `sync`: also asks for a quiet sync (linked phone only; the backend skips it if
 * the last one is very recent). Returns the unsubscribe function.
 */
export function watchHistory(reload: () => void, sync = true): () => void {
  const unlisten = events.onHistorySynced(() => reload());
  if (sync && platform.mobile) {
    commands
      .syncHistory(false)
      .then((at) => {
        if (at !== null && handoff.linked) handoff.linked = { ...handoff.linked, syncedAt: at };
      })
      .catch((e) => console.info("history not synced", e));
  }
  return () => void unlisten.then((u) => u());
}
