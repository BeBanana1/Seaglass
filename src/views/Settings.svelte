<!-- Settings page: one section per tab, in `settings/`. Every value comes from the backend. -->
<script lang="ts">
  import { commands, events } from "../lib/ipc";
  import { replace, type SettingsSection } from "../lib/stores/nav.svelte";
  import { t } from "../lib/i18n.svelte";
  import { platform } from "../lib/platform.svelte";
  import { playbackSettingsChanged } from "../lib/stores/palette.svelte";
  import { fail, SettingsState } from "./settings/state.svelte";
  import Appearance from "./settings/Appearance.svelte";
  import PlaybackOutput from "./settings/PlaybackOutput.svelte";
  import PlaybackListening from "./settings/PlaybackListening.svelte";
  import AppSystem from "./settings/AppSystem.svelte";
  import AppData from "./settings/AppData.svelte";
  import AppAbout from "./settings/AppAbout.svelte";

  let { section, onLogout }: { section: SettingsSection; onLogout?: () => void } = $props();

  // Android (#24): no audio output or cache (mpv settings), quality chosen among TIDAL's
  // tiers (#366), no window, launch at startup or updates (desktop settings).
  const desktop = $derived(!platform.mobile);

  const sections: SettingsSection[] = ["appearance", "playback", "app"];

  // Shared by the tabs, for as long as this page is mounted.
  const s = new SettingsState();

  // Exclusive mode changed from the palette (#412): reread, otherwise the next setting
  // saved here would put the old value back.
  $effect(() => {
    if (playbackSettingsChanged.count > 0) void s.loadPlayback();
  });

  // Output device profile applied by the backend (#416): normalization and exclusive mode
  // may have changed.
  $effect(() => {
    let unlisten: (() => void) | undefined;
    let alive = true;
    void events
      .onPlaybackSettings(() => void s.loadPlayback())
      .then((u) => {
        if (alive) unlisten = u;
        else u();
      });
    return () => {
      alive = false;
      unlisten?.();
    };
  });

  $effect(() => {
    if (section === "playback" && s.playback === null) s.loadPlayback();
    if (section === "app" && s.autostart === null && desktop) s.loadAutostart();
  });

  $effect(() => {
    if (section === "playback" && desktop) {
      commands.exclusiveAvailable().then((v) => (s.exclusiveAvailable = v), fail);
    }
  });

  $effect(() => {
    if (section === "app" && desktop) {
      commands.discordAvailable().then((v) => (s.discordAvailable = v), fail);
      commands
        .globalShortcutsStatus()
        .then((st) => (s.globalShortcutsAvailable = st.available), fail);
      commands.updatesAvailable().then((v) => (s.updatesAvailable = v), fail);
    }
  });

  $effect(() => {
    if (section === "app") commands.updateServer().then((v) => (s.updateServer = v), fail);
  });

  $effect(() => {
    if (section !== "app" || desktop) return;
    const load = () => commands.linkHandling().then((v) => (s.links = v), fail);
    load();
    // Back from the system setting: the state may have changed.
    const onVisible = () => document.visibilityState === "visible" && load();
    document.addEventListener("visibilitychange", onVisible);
    return () => document.removeEventListener("visibilitychange", onVisible);
  });

  // Android (#492): quality lowered by the battery right now. Reread when the settings
  // change (`playback` replaced) and on return to the foreground (battery, power saver).
  $effect(() => {
    if (section !== "playback" || desktop || s.playback === null) return;
    const load = () => commands.qualityCap().then((v) => (s.qualityCap = v), fail);
    load();
    const onVisible = () => document.visibilityState === "visible" && load();
    document.addEventListener("visibilitychange", onVisible);
    return () => document.removeEventListener("visibilitychange", onVisible);
  });

  // Desktop (#187): the check answers before the installer has finished downloading.
  $effect(() => {
    const ready = events.onUpdateReady(() => {
      if (s.updateCheck === "downloading") s.updateCheck = "ready";
    });
    const failed = events.onUpdateFailed((message) => {
      if (s.updateCheck !== "downloading") return;
      s.updateCheck = "idle";
      fail(t.settings.checkFailed(message));
    });
    return () => {
      void ready.then((f) => f());
      void failed.then((f) => f());
    };
  });
</script>

<section class="settings">
  <h1>{t.settings.title}</h1>
  <div class="tabs" role="tablist">
    {#each sections as id (id)}
      <button
        role="tab"
        aria-selected={id === section}
        class:active={id === section}
        onclick={() => replace({ name: "settings", section: id })}
      >
        {t.settings.sections[id]}
      </button>
    {/each}
  </div>

  {#if section === "appearance"}
    <Appearance />
  {:else if section === "playback"}
    <PlaybackOutput {s} />
    <PlaybackListening {s} />
  {:else}
    <AppSystem {s} />
    <AppData {s} />
    <AppAbout {s} {onLogout} />
  {/if}
</section>

<style>
  .settings {
    padding: 24px;
    max-width: 1040px;
  }

  h1 {
    margin: 0 10px;
    font-size: 26px;
  }

  .tabs {
    display: flex;
    gap: 6px;
    margin: 16px 10px 20px;
  }

  .tabs button {
    padding: 6px 14px;
    border-radius: 999px;
    color: var(--text-muted);
    background: var(--bg-raised);
  }

  .tabs button.active {
    color: var(--bg);
    background: var(--text);
  }

  /* Studio: sharp corners. */
  :global([data-theme="studio"]) .tabs button {
    border-radius: 0;
  }
</style>
