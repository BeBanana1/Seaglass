// Jam (#25): the Jam's state, relay connection and guest names, received from the backend.
// On Android, online only (no Jam on the local network).

import { commands, events, type JamMode, type JamRules, type JamStatus } from "../ipc";
import { setUiToggle, ui } from "./ui.svelte";

export const jam = $state<{
  status: JamStatus | null;
  busy: boolean;
  onlineAvailable: boolean;
  /** Quick access dialog opened from "Now playing" (JamDialog). */
  dialogOpen: boolean;
}>({
  status: null,
  busy: false,
  onlineAvailable: false,
  dialogOpen: false,
});

/** The Jam is offered: turned on in Settings (#452), and desktop (local network at
 *  least) or a usable relay. */
export function jamAvailable(desktop: boolean): boolean {
  return ui.settings.jam && (desktop || jam.onlineAvailable);
}

/** Relay changed in Settings (#452): is the online Jam still offered? */
export async function refreshJamOnline() {
  jam.onlineAvailable = await commands.jamOnlineAvailable();
}

/** "Jam" switch (#452): when turned off, the backend stops the current one. */
export async function setJamEnabled(enabled: boolean) {
  await setUiToggle("jam", enabled);
  if (!enabled) {
    jam.status = null;
    jam.dialogOpen = false;
  }
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
  // Relay connection: connected, reconnecting, new address.
  void events.onJamStatus((status) => {
    if (jam.status) jam.status = status;
  });
  try {
    [jam.status, jam.onlineAvailable] = await Promise.all([
      commands.jamStatus(),
      commands.jamOnlineAvailable(),
    ]);
  } catch {
    // Jam unavailable: nothing to show.
  }
}

/** Guest who added this track to the queue during the current Jam. */
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

/** Host rules (#222, #288): saved, then applied to the current Jam. */
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
