// Transfert de la lecture avec l'ordinateur lié (#414), côté téléphone, et historique
// d'écoute synchronisé (#430). Le backend garde le lien (Keystore) et fait les échanges ;
// ce store ne retient que ce que l'UI affiche.

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

/** Lit l'ordinateur lié une fois (Android seulement : le bureau n'a pas de bouton). */
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

/** « Reprendre ici » : la lecture de l'ordinateur passe sur le téléphone. */
export const pullHere = () => run(commands.handoffPull, t.handoff.pulled);

/** « Continuer sur l'ordinateur ». */
export const pushToComputer = () => run(commands.handoffPush, t.handoff.pushed);

/** « Synchroniser » de Réglages : historique d'écoute échangé tout de suite (#430). */
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
 * Vues de l'historique (accueil, statistiques, rétrospective) : `reload` quand des écoutes
 * arrivent de l'autre appareil. `sync` : demande aussi une synchro discrète (téléphone
 * lié seulement ; le backend l'ignore si la dernière est toute récente). Renvoie de quoi
 * se désabonner.
 */
export function watchHistory(reload: () => void, sync = true): () => void {
  const unlisten = events.onHistorySynced(() => reload());
  if (sync && platform.mobile) {
    commands
      .syncHistory(false)
      .then((at) => {
        if (at !== null && handoff.linked) handoff.linked = { ...handoff.linked, syncedAt: at };
      })
      .catch((e) => console.info("historique non synchronisé", e));
  }
  return () => void unlisten.then((u) => u());
}
