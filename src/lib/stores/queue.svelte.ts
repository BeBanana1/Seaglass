// Miroir de la file d'attente envoyée par le backend (`queue-state`).

import { commands, events, type QueueView } from "../ipc";

export const queue = $state<{ view: QueueView }>({ view: { manual: [], upcoming: [] } });

export async function initQueue() {
  await events.onQueueState((v) => (queue.view = v));
  queue.view = await commands.queueState();
}
