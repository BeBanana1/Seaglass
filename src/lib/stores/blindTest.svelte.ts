// Blind test (#434) : miroir de la partie tenue par le backend (`blind-test`), et liste
// choisie pour la prochaine partie. Aucune logique de jeu ici.

import { commands, events, type BlindTestView, type TrackId } from "../ipc";

/** Liste d'où tirer les manches, choisie depuis l'en-tête d'une page. */
export interface BlindPool {
  title: string;
  trackIds: TrackId[];
}

export const blindTest = $state<{ view: BlindTestView | null; pool: BlindPool | null }>({ view: null, pool: null });

/** Titres nécessaires pour proposer une partie (quatre propositions par manche). */
export const BLIND_MIN_TRACKS = 4;

export async function initBlindTest() {
  await events.onBlindTest((view) => (blindTest.view = view));
  blindTest.view = await commands.blindTestState();
}
