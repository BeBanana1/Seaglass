// Mirror of the queue sent by the backend (`queue-state`).

import { commands, events, type QueueView } from "../ipc";

export const queue = $state<{ view: QueueView }>({
  view: { manual: [], upcoming: [], radio: null, source: null },
});

/** What the "end of queue" sleep timer (#436) targets, from the current track's source. */
export type SleepScope = "album" | "playlist" | "queue";

export function sleepScope(): SleepScope {
  const kind = queue.view.source?.kind;
  return kind === "album" || kind === "playlist" ? kind : "queue";
}

export async function initQueue() {
  await events.onQueueState((v) => (queue.view = v));
  queue.view = await commands.queueState();
}
