import { commands, events, type Toast } from "../ipc";
import { t } from "../i18n.svelte";
import { ui } from "./ui.svelte";

const LIFETIME_MS = 6000;
/** Toast avec action (#192) : le temps de lire et de cliquer. */
const ACTION_LIFETIME_MS = 15000;

/** Annulation (#349) : assez long pour réagir, sans encombrer. */
const UNDO_LIFETIME_MS = 7000;

/** Toast de l'UI : un clic exécute `action` s'il y en a une, puis le ferme. `actionLabel`
 *  l'annonce dans le toast (« Annuler ») ; `lifetimeMs` remplace la durée par défaut. */
export type UiToast = Toast & { action?: () => void; actionLabel?: string; lifetimeMs?: number };

export const toasts = $state<{ items: (UiToast & { id: number })[] }>({ items: [] });

let nextId = 0;

export function pushToast(t: UiToast) {
  const id = nextId++;
  toasts.items.push({ ...t, id });
  setTimeout(() => dismissToast(id), t.lifetimeMs ?? (t.action ? ACTION_LIFETIME_MS : LIFETIME_MS));
}

export function dismissToast(id: number) {
  toasts.items = toasts.items.filter((t) => t.id !== id);
}

/** Toast « … · Annuler » (#349) : un clic exécute `undo`. */
export function pushUndoToast(message: string, undo: () => void) {
  pushToast({
    level: "info",
    message,
    action: undo,
    actionLabel: t.common.undo,
    lifetimeMs: UNDO_LIFETIME_MS,
  });
}

/** Clin d'œil (#399), thème Ordinateur de bord : un refus du lecteur s'excuse d'une
 *  politesse inquiétante. Le message d'origine suit, tel quel, pour que l'erreur reste claire. */
function dressed(toast: Toast): Toast {
  if (!toast.kind || !ui.settings.easterEggs || ui.settings.theme !== "onboard") return toast;
  return { ...toast, message: `${t.eggs.refusals[toast.kind]} ${toast.message}` };
}

export async function initToasts() {
  await events.onToast((toast) => pushToast(dressed(toast)));
}

/** Copie un texte du backend ; `null` = indisponible. */
export async function copyText(load: () => Promise<string | null>, done: string, missing: string) {
  try {
    const text = await load();
    if (text === null) {
      pushToast({ level: "error", message: missing });
      return;
    }
    await navigator.clipboard.writeText(text);
    pushToast({ level: "info", message: done });
  } catch (e) {
    pushToast({ level: "error", message: String(e) });
  }
}

/** Au lancement (#192) : plantage au lancement précédent, rapport copiable d'un clic. */
export async function noticeCrash() {
  if (!(await commands.takeCrashNotice().catch(() => false))) return;
  pushToast({
    level: "error",
    message: t.settings.crashed,
    action: () =>
      void copyText(commands.lastCrashReport, t.settings.crashCopied, t.settings.noCrash),
  });
}
