// Miroir de l'état du lecteur envoyé par le backend. Aucune logique métier ici.

import { untrack } from "svelte";
import { commands, events, type PlayerState, type Track, type TrackId } from "../ipc";
import { platform } from "../platform.svelte";

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

/** L'état est remplacé à chaque `player-state` (~4 Hz pendant la lecture) : `$state.raw`, sans
 *  proxy profond. Tout lecteur de `state` est réveillé à chaque position reçue : ce qui ne
 *  change qu'au changement de titre ou d'état vit dans des champs à part, écrits seulement par
 *  `apply` et qui ne préviennent leurs lecteurs que si leur valeur change. Une liste de
 *  10 000 titres lit `currentTrackId` (#88). Un `$derived` ne suffirait pas : Svelte vérifie
 *  quand même chacun de ses lecteurs à chaque changement de sa source. */
class PlayerStore {
  state = $state.raw<PlayerState>(initial);
  /** `performance.now()` à la réception du dernier état, pour interpoler la position. */
  receivedAt = 0;
  /** Titre en cours, même référence tant qu'il ne change pas. */
  currentTrack = $state.raw<Track | null>(null);
  currentTrackId = $state<TrackId | null>(null);
  /** Un titre est en lecture (ni en pause, ni arrêté) : fonds animés, mise à jour différée. */
  playing = $state(false);
  /** La position avance : ni pause, ni chargement, ni attente du cache, ni arrêt. */
  advancing = $state(false);
  /** Clins d'œil (#399) : bande originale en cours, file terminée. */
  soundtrack = $state(false);
  ended = $state(false);
}

export const player = new PlayerStore();

/** Champs objets gardés par référence s'ils n'ont pas changé : `currentTrack`, ou un `$derived`
 *  qui lit `player.state.stream`, ne réveillent alors pas leurs lecteurs à chaque position. */
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
  player.receivedAt = performance.now();
  player.state = next;
  // Une valeur égale à la précédente ne réveille personne.
  player.currentTrack = next.track;
  player.currentTrackId = next.track?.id ?? null;
  player.playing = !!next.track && !next.paused && !next.idle;
  player.advancing = !next.paused && !next.buffering && !next.loading && !next.idle;
  player.soundtrack = next.soundtrack;
  player.ended = next.ended;
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
  const elapsed = player.advancing ? (now - player.receivedAt) / 1000 : 0;
  return Math.min(s.timePos + elapsed, s.duration || Infinity);
}

/** Page affichée. Cachée (écran éteint, autre app devant, fenêtre réduite), personne ne
 *  regarde la position : `followPosition` ne garde alors aucun minuteur (#324). */
const page = $state({ visible: typeof document === "undefined" || !document.hidden });
if (typeof document !== "undefined") {
  document.addEventListener("visibilitychange", () => (page.visible = !document.hidden));
}

/** Suit la position estimée : `cb` à chaque image tant que la lecture avance et que la page
 *  est affichée, une seule fois par état reçu sinon (pause, chargement, arrêt, page cachée :
 *  aucune boucle d'animation). À appeler dans un `$effect`, qui la relance à chaque état et
 *  au retour de la page ; renvoie le nettoyage de la boucle.
 *  Sur Android, `mobileHz` fois par seconde plutôt qu'à chaque image (#146) : une barre qui
 *  avance de 2 px/s n'a pas besoin de 120 mises à jour par seconde sur un écran à 120 Hz. */
export function followPosition(
  cb: (position: number) => void,
  mobileHz = 4,
): (() => void) | undefined {
  void player.state;
  const visible = page.visible;
  const position = estimatedPosition(performance.now());
  // Ce que lit `cb` (ligne de paroles courante…) ne doit pas relancer la boucle.
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
