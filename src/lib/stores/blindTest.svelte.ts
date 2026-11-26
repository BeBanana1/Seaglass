// Blind test (#434): mirror of the game held by the backend (`blind-test`), and the list
// chosen for the next game. No game logic here.

import { commands, events, type BlindTestView, type TrackId } from "../ipc";

/** List to draw the rounds from, chosen from a page's header. */
export interface BlindPool {
  title: string;
  trackIds: TrackId[];
}

export const blindTest = $state<{ view: BlindTestView | null; pool: BlindPool | null }>({
  view: null,
  pool: null,
});

/** Tracks needed to offer a game (four choices per round). */
export const BLIND_MIN_TRACKS = 4;

export async function initBlindTest() {
  await events.onBlindTest((view) => (blindTest.view = view));
  blindTest.view = await commands.blindTestState();
}
