// Jam locale (#25), bureau seulement : état de la Jam et noms des invités, reçus du backend.

import { commands, events, type JamStatus } from "../ipc";
import { platform } from "../platform.svelte";

export const jam = $state<{ status: JamStatus | null; busy: boolean }>({ status: null, busy: false });

export async function initJam() {
  if (platform.mobile) return;
  void events.onJamGuests((guests) => {
    if (jam.status) jam.status = { ...jam.status, guests };
  });
  try {
    jam.status = await commands.jamStatus();
  } catch {
    // Jam indisponible : rien à afficher.
  }
}

/** Invité qui a ajouté ce titre à la file pendant la Jam en cours. */
export function guestOf(trackId: number): string | undefined {
  return jam.status?.guests[trackId];
}

export async function startJam() {
  jam.busy = true;
  try {
    jam.status = await commands.startJam();
  } finally {
    jam.busy = false;
  }
}

export async function stopJam() {
  await commands.stopJam();
  jam.status = null;
}
