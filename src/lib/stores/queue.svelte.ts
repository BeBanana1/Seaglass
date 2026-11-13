// Miroir de la file d'attente envoyée par le backend (`queue-state`).

import { commands, events, type QueueView } from "../ipc";

export const queue = $state<{ view: QueueView }>({ view: { manual: [], upcoming: [], radio: null, source: null } });

/** Ce que vise le minuteur « fin de la file » (#436), d'après la source du titre en cours. */
export type SleepScope = "album" | "playlist" | "queue";

export function sleepScope(): SleepScope {
  const kind = queue.view.source?.kind;
  return kind === "album" || kind === "playlist" ? kind : "queue";
}

export async function initQueue() {
  await events.onQueueState((v) => (queue.view = v));
  queue.view = await commands.queueState();
}
