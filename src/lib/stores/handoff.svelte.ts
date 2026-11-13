// Transfert de la lecture avec l'ordinateur lié (#414), côté téléphone. Le backend garde
// le lien (Keystore) et fait les échanges ; ce store ne retient que ce que l'UI affiche.

import { commands, type LinkedComputer } from "../ipc";
import { t } from "../i18n.svelte";
import { platform } from "../platform.svelte";
import { pushToast } from "./toasts.svelte";

export const handoff = $state<{ linked: LinkedComputer | null; loaded: boolean; busy: boolean }>({
  linked: null,
  loaded: false,
  busy: false,
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
