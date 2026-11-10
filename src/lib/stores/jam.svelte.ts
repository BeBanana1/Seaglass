// Jam (#25) : état de la Jam, connexion au relais et noms des invités, reçus du backend.
// Sur Android, seulement en ligne (pas de Jam sur le réseau local).

import { commands, events, type JamMode, type JamRules, type JamStatus } from "../ipc";

export const jam = $state<{
  status: JamStatus | null;
  busy: boolean;
  onlineAvailable: boolean;
  /** Fenêtre d'accès rapide ouverte depuis « En lecture » (JamDialog). */
  dialogOpen: boolean;
}>({
  status: null,
  busy: false,
  onlineAvailable: false,
  dialogOpen: false,
});

/** La Jam est proposée : bureau (réseau local au moins) ou relais compilé. */
export function jamAvailable(desktop: boolean): boolean {
  return desktop || jam.onlineAvailable;
}

export function openJamDialog() {
  jam.dialogOpen = true;
}

export function closeJamDialog() {
  jam.dialogOpen = false;
}

export async function initJam() {
  void events.onJamGuests(({ guests, members }) => {
    if (jam.status) jam.status = { ...jam.status, guests, members };
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

/** Règles de l'hôte (#222, #288) : enregistrées, puis appliquées à la Jam en cours. */
export async function setJamRules(rules: JamRules) {
  jam.status = await commands.setJamRules(rules);
}

export async function setJamAddsOpen(open: boolean) {
  jam.status = await commands.setJamAddsOpen(open);
}

export async function setJamGuestBanned(id: number, banned: boolean) {
  jam.status = await commands.setJamGuestBanned(id, banned);
}

export async function stopJam() {
  await commands.stopJam();
  jam.status = null;
}
