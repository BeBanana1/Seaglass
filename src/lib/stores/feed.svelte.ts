// New items in the feed (#415), mirror of the counter held by the backend (`feed-unseen`
// event, emitted after each read of the feed and every 6 h): the navigation's badge.

import { events } from "../ipc";

export const feed = $state<{ unseen: number }>({ unseen: 0 });

export async function initFeed() {
  await events.onFeedUnseen((count) => {
    feed.unseen = count;
  });
}

/** On sign-out. */
export function clearFeed() {
  feed.unseen = 0;
}
