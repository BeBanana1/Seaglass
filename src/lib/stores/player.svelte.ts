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
  shuffle: false,
  repeat: "off",
  hasPrevious: false,
  hasNext: false,
  volume: { level: 100, muted: false },
  hiresOffer: null,
  mpvIssue: null,
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

/** Cran de volume pour la molette et Ctrl+↑ / Ctrl+↓. */
const VOLUME_STEP = 5;
/** Dernier niveau demandé, tant que le backend ne l'a pas forcément renvoyé (molette rapide). */
let pendingVolume: number | null = null;
let pendingTimer: ReturnType<typeof setTimeout> | undefined;

/** Monte (`1`) ou baisse (`-1`) le volume d'un cran, à partir du niveau affiché (0 si coupé). */
export function stepVolume(direction: 1 | -1) {
  const v = player.state.volume;
  if (v.muted && direction < 0 && pendingVolume === null) return;
  const base = pendingVolume ?? (v.muted ? 0 : v.level);
  const next = Math.max(0, Math.min(100, base + direction * VOLUME_STEP));
  pendingVolume = next;
  clearTimeout(pendingTimer);
  pendingTimer = setTimeout(() => (pendingVolume = null), 400);
  void commands.setVolume(next);
}

/** Position estimée à l'instant `now`, entre deux événements (~4 Hz). */
export function estimatedPosition(now: number): number {
  const s = player.state;
  const running = !s.paused && !s.buffering && !s.loading && !s.idle;
  const elapsed = running ? (now - player.receivedAt) / 1000 : 0;
  return Math.min(s.timePos + elapsed, s.duration || Infinity);
}
