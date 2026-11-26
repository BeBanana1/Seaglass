// State shared by the Settings tabs. `Settings.svelte` creates one per mount and passes it
// to every tab: it lives exactly as long as the page, so switching tabs keeps the playback
// settings, a running update check, a pasted import or an open changelog.

import { commands, type PlaybackSettings, type QualityCap } from "../../lib/ipc";
import { t } from "../../lib/i18n.svelte";
import { pushToast } from "../../lib/stores/toasts.svelte";
import { ui } from "../../lib/stores/ui.svelte";

export type UpdateCheck = "idle" | "checking" | "uptodate" | "downloading" | "ready";

export function fail(e: unknown) {
  pushToast({ level: "error", message: String(e) });
}

export class SettingsState {
  playback = $state<PlaybackSettings | null>(null);
  autostart = $state<boolean | null>(null);
  autostartBusy = $state(false);
  discordAvailable = $state(false);
  // Global shortcuts (#413): offered by the system (not on Linux without an X server).
  globalShortcutsAvailable = $state(false);
  changelogOpen = $state(false);
  /** Easter egg (#399): 7 clicks in a row on the version start the end credits. */
  creditsOpen = $state(false);
  #versionClicks = 0;
  #versionTimer: ReturnType<typeof setTimeout> | undefined;
  // False on Linux outside an AppImage (#314): no "Check" button.
  updatesAvailable = $state(true);
  // Built-in update folder (#452); `null`: no updates, nothing to offer.
  updateServer = $state<string | null>(null);
  exclusiveAvailable = $state(false);
  // Android 12+ (#203): TIDAL links given to the app; `null` = nothing to set or not read yet.
  links = $state<{ enabled: boolean } | null>(null);
  // Android (#492): quality lowered by the battery right now.
  qualityCap = $state<QualityCap | null>(null);
  updateCheck = $state<UpdateCheck>("idle");
  downloadingVersion = $state("");
  // #352: settings backup through the clipboard; `null` = import field closed.
  importText = $state<string | null>(null);
  importing = $state(false);

  clickVersion = () => {
    clearTimeout(this.#versionTimer);
    this.#versionTimer = setTimeout(() => (this.#versionClicks = 0), 1500);
    if (++this.#versionClicks < 7) return;
    this.#versionClicks = 0;
    if (ui.settings.easterEggs) this.creditsOpen = true;
  };

  loadPlayback = async () => {
    try {
      this.playback = await commands.playbackSettings();
    } catch (e) {
      fail(e);
    }
  };

  savePlayback = async (next: PlaybackSettings) => {
    try {
      await commands.setPlaybackSettings(next);
      this.playback = next;
    } catch (e) {
      fail(e);
    }
  };

  loadAutostart = async () => {
    try {
      this.autostart = await commands.autostartEnabled();
    } catch (e) {
      fail(e);
    }
  };

  toggleAutostart = async () => {
    if (this.autostart === null || this.autostartBusy) return;
    this.autostartBusy = true;
    try {
      this.autostart = await commands.setAutostart(!this.autostart);
    } catch (e) {
      fail(e);
      await this.loadAutostart();
    } finally {
      this.autostartBusy = false;
    }
  };

  checkUpdates = async () => {
    if (this.updateCheck === "checking" || this.updateCheck === "downloading") return;
    this.updateCheck = "checking";
    try {
      // A version already downloaded is sent again as `update-ready`: UpdatePrompt offers it.
      const info = await commands.checkForUpdates();
      if (info?.downloading) this.downloadingVersion = info.version;
      this.updateCheck = !info ? "uptodate" : info.downloading ? "downloading" : "ready";
    } catch (e) {
      this.updateCheck = "idle";
      fail(t.settings.checkFailed(String(e)));
    }
  };

  applyImport = async () => {
    if (this.importText === null || !this.importText.trim() || this.importing) return;
    this.importing = true;
    try {
      const report = await commands.importSettings(this.importText);
      pushToast({
        level: "info",
        message: t.settings.settingsImported(report.applied.length, report.ignored),
      });
      this.importText = null;
      // Reread everything from the backend: theme, language, pins, playback settings.
      setTimeout(() => location.reload(), 1500);
    } catch (e) {
      fail(e);
    } finally {
      this.importing = false;
    }
  };
}
