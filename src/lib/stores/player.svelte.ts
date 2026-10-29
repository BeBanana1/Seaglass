// Miroir de l'état du lecteur envoyé par le backend. Aucune logique métier ici.

import { commands, events, type PlayerState } from "../ipc";

const initial: PlayerState = {
  track: null,
  stream: null,
  output: null,
  outputDriver: null,
  loading: false,
  paused: false,
  buffering: false,
  idle: true,
  timePos: 0,
  duration: 0,
};

export const player = $state({
  state: initial,
  /** `performance.now()` à la réception du dernier état, pour interpoler la position. */
  receivedAt: 0,
});

function apply(s: PlayerState) {
  player.state = s;
  player.receivedAt = performance.now();
}

export async function initPlayer() {
  await events.onPlayerState(apply);
  apply(await commands.playerState());
}

/** Position estimée à l'instant `now`, entre deux événements (~4 Hz). */
export function estimatedPosition(now: number): number {
  const s = player.state;
  const running = !s.paused && !s.buffering && !s.loading && !s.idle;
  const elapsed = running ? (now - player.receivedAt) / 1000 : 0;
  return Math.min(s.timePos + elapsed, s.duration || Infinity);
}
