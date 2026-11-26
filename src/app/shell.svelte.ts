// État partagé par la coquille de l'app. `App.svelte` en crée un par montage et le passe aux
// modules de `app/` : session, panneaux ouverts, saisie sur Android, reprise de session.

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
  /** Mode conduite (#193, Android) : plein écran au-dessus de tout le reste. */
  driveOpen = $state(false);
  /** « En lecture » ouvert par la notification d'Android (#240) : la page dessous est quelconque. */
  nowPlayingFromOutside = false;
  // Android : un champ de texte a le focus, donc le clavier est ouvert (#151).
  typing = $state(false);

  // Session enregistrée non reprise à cause d'une erreur passagère (réseau absent au
  // lancement avec Windows, panne de TIDAL) : le refresh token est gardé, on réessaie au
  // lieu de demander une reconnexion (#252).
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

  // Ouvert depuis la notification, « En lecture » se referme sur la page d'où joue le titre
  // (#240), par-dessus la page qui était là : Retour y revient ensuite.
  closeNowPlaying = () => {
    this.nowPlayingOpen = false;
    if (!this.nowPlayingFromOutside) return;
    this.nowPlayingFromOutside = false;
    const route = playedFrom(queue.view.source)?.route;
    if (route && JSON.stringify(route) !== JSON.stringify(current())) go(route);
  };
}
