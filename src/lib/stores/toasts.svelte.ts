import { events, type Toast } from "../ipc";

const LIFETIME_MS = 6000;

export const toasts = $state<{ items: (Toast & { id: number })[] }>({ items: [] });

let nextId = 0;

export function pushToast(t: Toast) {
  const id = nextId++;
  toasts.items.push({ ...t, id });
  setTimeout(() => dismissToast(id), LIFETIME_MS);
}

export function dismissToast(id: number) {
  toasts.items = toasts.items.filter((t) => t.id !== id);
}

export async function initToasts() {
  await events.onToast(pushToast);
}
