// Requests from outside the app: notification, `seaglass://` links, pins, shared links.
// Rust keeps each one until the UI takes it, once.

import type { Shell } from "./shell.svelte";
import { commands, events } from "../lib/ipc";
import { player } from "../lib/stores/player.svelte";
import { playPin } from "../lib/pins";
import { pushToast } from "../lib/stores/toasts.svelte";
import { platform } from "../lib/platform.svelte";
import { openSharedLink } from "../lib/links";

// Tap on the Android notification (#163): request kept by Rust, taken once (also at
// startup, if the notification reopened the app).
async function openFromNotification(shell: Shell) {
  if (!(await commands.takeNowPlayingRequest().catch(() => false))) return;
  if (shell.session && player.state.track) {
    shell.nowPlayingOpen = true;
    shell.nowPlayingFromOutside = true;
  }
}

// `seaglass://drive` link or app shortcut (#193): request kept by Rust, taken once (also
// at startup). Signed out, it is dropped: there is nothing to play there.
async function openDriveFromOutside(shell: Shell) {
  if (!(await commands.takeDriveRequest().catch(() => false))) return;
  if (!shell.session) return;
  shell.nowPlayingOpen = false;
  shell.driveOpen = true;
}

// Pin shortcut (long press on the icon) or `seaglass://pin/…` link (#335): the pin plays
// like a driving mode tile, then "Now playing" opens. Signed out, the request is dropped.
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

// Link shared from another app (#203): only when signed in, otherwise it waits.
export async function openShared(shell: Shell) {
  if (!shell.session) return;
  shell.nowPlayingOpen = false;
  await openSharedLink();
}

/** Listens for the requests and takes those already waiting at startup. */
export function listenOutside(shell: Shell) {
  // Shared link (Android, #203) or a tapped new releases notification (#435).
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
