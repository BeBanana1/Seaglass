// Mirror of the player state sent by the backend. No business logic here.

import { untrack } from "svelte";
import { commands, events, type PlayerState, type Track, type TrackId } from "../ipc";
import { platform } from "../platform.svelte";
import { clockPosition, clockSample, type PositionClock } from "../positionClock";

const initial: PlayerState = {
  track: null,
  stream: null,
  output: null,
  deviceFormat: null,
  outputDriver: null,
  exclusive: false,
  outputDevice: null,
  loading: false,
  paused: false,
  buffering: false,
  idle: true,
  timePos: 0,
  duration: 0,
  cached: 0,
  nextReady: false,
  shuffle: false,
  repeat: "off",
  hasPrevious: false,
  hasNext: false,
  volume: { level: 100, muted: false },
  hiresOffer: null,
  normalization: null,
  equalizer: null,
  sleep: null,
  mpvIssue: null,
  soundtrack: false,
  ended: false,
  blindTest: false,
};

/** The state is replaced on every `player-state` (~4 Hz during playback): `$state.raw`, no
 *  deep proxy. Every reader of `state` wakes up on each position received: what only changes
 *  with the track or the playback state lives in separate fields, written only by `apply`,
 *  which notify their readers only when their value changes. A 10,000-track list reads
 *  `currentTrackId` (#88). A `$derived` would not do: Svelte still checks each of its readers
 *  whenever its source changes. */
class PlayerStore {
  state = $state.raw<PlayerState>(initial);
  /** Interpolated position between two states, without roll-backs (#501). */
  clock: PositionClock | null = null;
  /** Current track, same reference as long as it does not change. */
  currentTrack = $state.raw<Track | null>(null);
  currentTrackId = $state<TrackId | null>(null);
  /** A track is playing (neither paused nor stopped): animated backgrounds, deferred update. */
  playing = $state(false);
  /** The position advances: not paused, loading, waiting for the cache or stopped. */
  advancing = $state(false);
  /** Easter eggs (#399): soundtrack playing, queue ended. */
  soundtrack = $state(false);
  ended = $state(false);
}

export const player = new PlayerStore();

/** Object fields kept by reference when unchanged: `currentTrack`, or a `$derived` reading
 *  `player.state.stream`, then do not wake their readers on every position. */
const SHARED = ["track", "stream", "output", "volume", "hiresOffer", "sleep"] as const;

function share(prev: PlayerState, next: PlayerState): PlayerState {
  const out = { ...next };
  for (const key of SHARED) {
    const a = prev[key];
    const b = next[key];
    if (a && b && a !== b && JSON.stringify(a) === JSON.stringify(b)) {
      (out as Record<string, unknown>)[key] = a;
    }
  }
  return out;
}

function apply(s: PlayerState) {
  const next = share(player.state, s);
  player.state = next;
  // A value equal to the previous one wakes nobody.
  player.currentTrack = next.track;
  player.currentTrackId = next.track?.id ?? null;
  player.playing = !!next.track && !next.paused && !next.idle;
  player.advancing = !next.paused && !next.buffering && !next.loading && !next.idle;
  player.clock = clockSample(
    player.clock,
    { pos: next.timePos, running: player.advancing, key: next.track?.id ?? null },
    performance.now(),
  );
  player.soundtrack = next.soundtrack;
  player.ended = next.ended;
}

export async function initPlayer() {
  await events.onPlayerState(apply);
  apply(await commands.playerState());
}

/** Volume step for the mouse wheel and Ctrl+↑ / Ctrl+↓. */
const VOLUME_STEP = 5;
/** Last level requested, while the backend may not have sent it back yet (fast wheel). */
let pendingVolume: number | null = null;
let pendingTimer: ReturnType<typeof setTimeout> | undefined;

/** Raises (`1`) or lowers (`-1`) the volume by one step, from the shown level (0 if muted). */
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

/** Estimated position at `now`, between two events (~4 Hz). */
export function estimatedPosition(now: number): number {
  const s = player.state;
  const pos = player.clock ? clockPosition(player.clock, now) : s.timePos;
  return Math.min(pos, s.duration || Infinity);
}

/** Page shown. When hidden (screen off, another app in front, window minimized), nobody looks
 *  at the position: `followPosition` then keeps no timer (#324). */
const page = $state({ visible: typeof document === "undefined" || !document.hidden });
if (typeof document !== "undefined") {
  document.addEventListener("visibilitychange", () => (page.visible = !document.hidden));
}

/** Follows the estimated position: `cb` on every frame while playback advances and the page
 *  is shown, otherwise once per state received (pause, loading, stop, hidden page: no
 *  animation loop). To be called in an `$effect`, which restarts it on every state and when the
 *  page comes back; returns the loop's cleanup.
 *  On Android, `mobileHz` times per second rather than on every frame (#146): a bar moving
 *  2 px/s does not need 120 updates per second on a 120 Hz screen. */
export function followPosition(
  cb: (position: number) => void,
  mobileHz = 4,
): (() => void) | undefined {
  void player.state;
  const visible = page.visible;
  const position = estimatedPosition(performance.now());
  // What `cb` reads (current lyrics line…) must not restart the loop.
  untrack(() => cb(position));
  if (!player.advancing || !visible) return;
  if (platform.mobile) {
    const timer = setInterval(() => cb(estimatedPosition(performance.now())), 1000 / mobileHz);
    return () => clearInterval(timer);
  }
  let frame = requestAnimationFrame(function tick(now) {
    cb(estimatedPosition(now));
    frame = requestAnimationFrame(tick);
  });
  return () => cancelAnimationFrame(frame);
}
