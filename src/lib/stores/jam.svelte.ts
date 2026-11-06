// Jam (#25) : état de la Jam, connexion au relais et noms des invités, reçus du backend.
// Sur Android, seulement en ligne (pas de Jam sur le réseau local).

import { commands, events, type JamMode, type JamStatus } from "../ipc";

export const jam = $state<{ status: JamStatus | null; busy: boolean; onlineAvailable: boolean }>({
  status: null,
  busy: false,
  onlineAvailable: false,
});

export async function initJam() {
  void events.onJamGuests((guests) => {
    if (jam.status) jam.status = { ...jam.status, guests };
  });
  // Connexion au relais : connecté, reconnexion, nouvelle adresse.
  void events.onJamStatus((status) => {
    if (jam.status) jam.status = status;
  });
  try {
    [jam.status, jam.onlineAvailable] = await Promise.all([
      commands.jamStatus(),
      commands.jamOnlineAvailable(),
    ]);
  } catch {
    // Jam indisponible : rien à afficher.
  }
}

/** Invité qui a ajouté ce titre à la file pendant la Jam en cours. */
export function guestOf(trackId: number): string | undefined {
  return jam.status?.guests[trackId];
}

export async function startJam(mode: JamMode) {
  jam.busy = true;
  try {
    jam.status = await commands.startJam(mode);
  } finally {
    jam.busy = false;
  }
}

export async function stopJam() {
  await commands.stopJam();
  jam.status = null;
}
