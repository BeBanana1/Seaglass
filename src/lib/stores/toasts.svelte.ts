import { commands, events, type Toast } from "../ipc";
import { t } from "../i18n.svelte";
import { ui } from "./ui.svelte";

const LIFETIME_MS = 6000;
/** Toast with an action (#192): time to read and click. */
const ACTION_LIFETIME_MS = 15000;

/** Undo (#349): long enough to react, without getting in the way. */
const UNDO_LIFETIME_MS = 7000;

/** UI toast: a click runs `action` if there is one, then closes it. `actionLabel` announces
 *  it in the toast ("Undo"); `lifetimeMs` replaces the default duration. */
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

/** "… · Undo" toast (#349): a click runs `undo`. */
export function pushUndoToast(message: string, undo: () => void) {
  pushToast({
    level: "info",
    message,
    action: undo,
    actionLabel: t.common.undo,
    lifetimeMs: UNDO_LIFETIME_MS,
  });
}

/** Easter egg (#399), Ship Computer theme: a refusal from the player apologizes with an
 *  unsettling politeness. The original message follows as is, so the error stays clear. */
function dressed(toast: Toast): Toast {
  if (!toast.kind || !ui.settings.easterEggs || ui.settings.theme !== "onboard") return toast;
  return { ...toast, message: `${t.eggs.refusals[toast.kind]} ${toast.message}` };
}

export async function initToasts() {
  await events.onToast((toast) => pushToast(dressed(toast)));
}

/** Copies a text from the backend; `null` = unavailable. */
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

/** At launch (#192): crash during the previous run, report copyable in one click. */
export async function noticeCrash() {
  if (!(await commands.takeCrashNotice().catch(() => false))) return;
  pushToast({
    level: "error",
    message: t.settings.crashed,
    action: () =>
      void copyText(commands.lastCrashReport, t.settings.crashCopied, t.settings.noCrash),
  });
}
