// State shared by the app's shell. `App.svelte` creates one per mount and passes it to the
// modules of `app/`: session, open panels, typing on Android, session restore.

import { commands, type SessionInfo } from "../lib/ipc";
import { queue } from "../lib/stores/queue.svelte";
import { playedFrom } from "../lib/playSource";
import { clearLikes } from "../lib/stores/likes.svelte";
import { clearLater } from "../lib/stores/later.svelte";
import { clearFeed } from "../lib/stores/feed.svelte";
import { clearFolders } from "../lib/stores/folders.svelte";
import { current, go } from "../lib/stores/nav.svelte";

export class Shell {
  session = $state<SessionInfo | null>(null);
  ready = $state(false);
  queueOpen = $state(false);
  nowPlayingOpen = $state(false);
  /** Driving mode (#193, Android): full screen above everything else. */
  driveOpen = $state(false);
  /** "Now playing" opened from the Android notification (#240): the page below is any page. */
  nowPlayingFromOutside = false;
  // Android: a text field has the focus, so the keyboard is open (#151).
  typing = $state(false);

  // Saved session not restored because of a passing error (no network yet when launched
  // with Windows, TIDAL outage): the refresh token is kept, so it retries instead of asking
  // to sign in again (#252).
  restoreError = $state<string | null>(null);
  #restoreTimer: ReturnType<typeof setTimeout> | undefined;
  #restoreDelay = 5_000;

  restoreSession = async () => {
    clearTimeout(this.#restoreTimer);
    try {
      const restored = await commands.authStatus();
      if (!this.session) this.session = restored;
      this.restoreError = null;
      this.#restoreDelay = 5_000;
    } catch (e) {
      this.restoreError = String(e);
      this.#restoreTimer = setTimeout(() => void this.restoreSession(), this.#restoreDelay);
      this.#restoreDelay = Math.min(this.#restoreDelay * 2, 60_000);
    }
  };

  signIn = (s: SessionInfo) => {
    clearTimeout(this.#restoreTimer);
    this.restoreError = null;
    this.session = s;
  };

  logout = async () => {
    await commands.logout();
    this.session = null;
    clearLikes();
    clearLater();
    clearFeed();
    clearFolders();
  };

  // Opened from the notification, "Now playing" closes onto the page the track plays from
  // (#240), on top of the page that was there: Back returns to it afterwards.
  closeNowPlaying = () => {
    this.nowPlayingOpen = false;
    if (!this.nowPlayingFromOutside) return;
    this.nowPlayingFromOutside = false;
    const route = playedFrom(queue.view.source)?.route;
    if (route && JSON.stringify(route) !== JSON.stringify(current())) go(route);
  };
}
