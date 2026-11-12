// Clins d'œil liés à la lecture et au clavier (#399, lot B) : logique pure, sans DOM.

import type { TrackId } from "../ipc";

/** Boucles du titre courant en Repeat One : `last` est la dernière position vue. */
export interface Loops {
  id: TrackId | null;
  count: number;
  last: number;
}

export const NO_LOOPS: Loops = { id: null, count: 0, last: 0 };

/** Après 9 retours au début, le titre a joué 10 fois de suite : l'icône passe à ∞. */
export const LOOPS_FOR_INFINITY = 9;

/** Compte un tour quand la position repart du début après être passée près de la fin, sur
 *  le même titre en Repeat One. Un autre titre ou un autre mode remet le compte à zéro ; un
 *  retour au début en pleine écoute (Précédent, recherche) ne compte pas. */
export function countLoops(
  prev: Loops,
  id: TrackId | null,
  pos: number,
  duration: number,
  repeatOne: boolean,
): Loops {
  if (!repeatOne || id === null || id !== prev.id) return { id, count: 0, last: pos };
  const wrapped = duration > 0 && prev.last >= duration - 5 && pos < 3;
  return { id, count: wrapped ? prev.count + 1 : prev.count, last: pos };
}

/** Séquence au clavier (↑↑↓↓←→←→BA), touches de `KeyboardEvent.key`. */
export const SEQUENCE = [
  "ArrowUp",
  "ArrowUp",
  "ArrowDown",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "ArrowLeft",
  "ArrowRight",
  "b",
  "a",
];

/** Avance dans la séquence : renvoie le nombre de touches justes, `SEQUENCE.length` quand
 *  elle est complète. Une touche fausse repart de zéro, ou de un si c'est un début. */
export function sequenceStep(progress: number, key: string): number {
  const k = key.length === 1 ? key.toLowerCase() : key;
  if (k === SEQUENCE[progress]) return progress + 1;
  return k === SEQUENCE[0] ? (progress === 2 ? 2 : 1) : 0;
}

/** Tour du cadran d'une horloge de gare : la trotteuse fait le tour en 58,5 s. */
const STATION_LAP = 58.5;

/** 1er août : position montrée par la barre de progression, comme la trotteuse d'une
 *  horloge de gare. Chaque minute est parcourue en 58,5 s, la barre s'arrête sur la minute
 *  pleine, puis repart quand la vraie minute est passée. Jamais au-delà de `duration`. */
export function stationClock(pos: number, duration: number): number {
  if (pos <= 0) return 0;
  const minute = Math.floor(pos / 60) * 60;
  const shown = minute + Math.min(((pos - minute) * 60) / STATION_LAP, 60);
  return duration > 0 ? Math.min(shown, duration) : shown;
}

/** Recherche « kessel » : une durée en parsecs, unité fantaisiste (une minute vaut un
 *  parsec), arrondie au dixième. */
export function parsecs(seconds: number): number {
  return Math.round((seconds / 60) * 10) / 10;
}
