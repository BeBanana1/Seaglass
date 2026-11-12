// Nouveautés du fil d'actualité (#415), miroir du compteur tenu par le backend (événement
// `feed-unseen`, émis après chaque lecture du fil et toutes les 6 h) : pastille de la
// navigation.

import { events } from "../ipc";

export const feed = $state<{ unseen: number }>({ unseen: 0 });

export async function initFeed() {
  await events.onFeedUnseen((count) => {
    feed.unseen = count;
  });
}

/** À la déconnexion. */
export function clearFeed() {
  feed.unseen = 0;
}
