// Demandes venues de l'extérieur de l'app : notification, liens `seaglass://`, épingles,
// liens partagés. Chacune est gardée par Rust et prise une seule fois.

import type { Shell } from "./shell.svelte";
import { commands, events } from "../lib/ipc";
import { player } from "../lib/stores/player.svelte";
import { playPin } from "../lib/pins";
import { pushToast } from "../lib/stores/toasts.svelte";
import { platform } from "../lib/platform.svelte";
import { openSharedLink } from "../lib/links";

// Appui sur la notification d'Android (#163) : demande gardée par Rust, prise une seule
// fois (aussi au démarrage, si l'app a été rouverte par la notification).
async function openFromNotification(shell: Shell) {
  if (!(await commands.takeNowPlayingRequest().catch(() => false))) return;
  if (shell.session && player.state.track) {
    shell.nowPlayingOpen = true;
    shell.nowPlayingFromOutside = true;
  }
}

// Lien `seaglass://drive` ou raccourci de l'app (#193) : demande gardée par Rust, prise une
// seule fois (aussi au démarrage). Déconnecté, elle est oubliée : rien à y lire.
async function openDriveFromOutside(shell: Shell) {
  if (!(await commands.takeDriveRequest().catch(() => false))) return;
  if (!shell.session) return;
  shell.nowPlayingOpen = false;
  shell.driveOpen = true;
}

// Raccourci d'une épingle (appui long sur l'icône) ou lien `seaglass://pin/…` (#335) :
// l'épingle est lue comme une tuile du mode conduite, puis « En lecture » s'ouvre.
// Déconnecté, la demande est oubliée.
async function playPinFromOutside(shell: Shell) {
  const pin = await commands.takePinRequest().catch(() => null);
  if (!pin || !shell.session) return;
  try {
    await playPin(pin, player.state.shuffle);
    shell.driveOpen = false;
    shell.nowPlayingOpen = true;
    shell.nowPlayingFromOutside = true;
  } catch (e) {
    pushToast({ level: "error", message: String(e) });
  }
}

// Lien partagé depuis une autre app (#203) : seulement connecté, sinon il attend.
export async function openShared(shell: Shell) {
  if (!shell.session) return;
  shell.nowPlayingOpen = false;
  await openSharedLink();
}

/** Écoute les demandes et prend celles déjà en attente au démarrage. */
export function listenOutside(shell: Shell) {
  // Lien partagé (Android, #203) ou notification des nouvelles sorties touchée (#435).
  void events.onSharedLink(() => openShared(shell));
  void openShared(shell);
  if (platform.mobile) {
    void events.onOpenNowPlaying(() => openFromNotification(shell));
    void openFromNotification(shell);
    void events.onOpenDrive(() => openDriveFromOutside(shell));
    void openDriveFromOutside(shell);
    void events.onOpenPin(() => playPinFromOutside(shell));
    void playPinFromOutside(shell);
  }
}
