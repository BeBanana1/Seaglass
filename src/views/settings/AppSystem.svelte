<!-- Settings › Application, first part: window, startup, notifications, integrations
     (Discord, ListenBrainz, global shortcuts), Jam, remote or linked computer, relay, links. -->
<script lang="ts">
  import { commands, type CloseAction } from "../../lib/ipc";
  import {
    setCloseAction,
    setDiscord,
    setGlobalShortcuts,
    setUiToggle,
    ui,
  } from "../../lib/stores/ui.svelte";
  import { t } from "../../lib/i18n.svelte";
  import { platform } from "../../lib/platform.svelte";
  import { jam, setJamEnabled } from "../../lib/stores/jam.svelte";
  import JamPanel from "../../components/JamPanel.svelte";
  import RelaySettings from "../../components/RelaySettings.svelte";
  import ListenBrainzSettings from "../../components/ListenBrainzSettings.svelte";
  import RemotePanel from "../../components/RemotePanel.svelte";
  import LinkedComputerPanel from "../../components/LinkedComputerPanel.svelte";
  import GlobalShortcutsList from "../../components/GlobalShortcutsList.svelte";
  import { fail, type SettingsState } from "./state.svelte";

  let { s }: { s: SettingsState } = $props();

  // Android (#24): no window, launch at startup or global shortcuts (desktop settings).
  const desktop = $derived(!platform.mobile);

  const CLOSE_ACTIONS: CloseAction[] = ["quit", "tray"];

  const autostart = $derived(s.autostart);
  const autostartBusy = $derived(s.autostartBusy);
  const discordAvailable = $derived(s.discordAvailable);
  const globalShortcutsAvailable = $derived(s.globalShortcutsAvailable);
  const links = $derived(s.links);

  function openLinkSettings() {
    commands.openLinkSettings().catch(fail);
  }

  async function toggleDiscord() {
    try {
      await setDiscord(!ui.settings.discord);
    } catch (e) {
      fail(e);
    }
  }

  async function toggleGlobalShortcuts() {
    try {
      const current = ui.settings.globalShortcuts;
      await setGlobalShortcuts({ ...current, enabled: !current.enabled });
    } catch (e) {
      fail(e);
    }
  }

  async function chooseCloseAction(action: CloseAction) {
    if (action === ui.settings.closeAction) return;
    try {
      await setCloseAction(action);
    } catch (e) {
      fail(e);
    }
  }
</script>

{#if desktop}
  <div class="group">
    <h2>{t.settings.window}</h2>
    <p class="hint">{t.settings.closeActionHint}</p>
    <div class="cache-sizes close-actions" role="radiogroup" aria-label={t.settings.closeAction}>
      {#each CLOSE_ACTIONS as action (action)}
        <button
          class="cache-size close-action"
          class:selected={ui.settings.closeAction === action}
          role="radio"
          aria-checked={ui.settings.closeAction === action}
          onclick={() => chooseCloseAction(action)}
        >
          {t.settings.closeActions[action]}
        </button>
      {/each}
    </div>
    <p class="note">{t.settings.closeActionNotes[ui.settings.closeAction]}</p>
  </div>

  <div class="group">
    <h2>{t.settings.startup}</h2>
    <label class="setting-row">
      <span class="text">
        <span class="setting-label">{t.settings.autostart}</span>
        <span class="hint">{t.settings.autostartHint}</span>
      </span>
      <button
        class="setting-switch"
        class:on={autostart === true}
        role="switch"
        aria-checked={autostart === true}
        aria-label={t.settings.autostart}
        disabled={autostart === null || autostartBusy}
        onclick={s.toggleAutostart}
      >
        <span class="knob"></span>
      </button>
    </label>
  </div>
{/if}

<!-- #435: notification of new releases from followed artists, off by default. -->
<div class="group">
  <h2>{t.settings.notifications}</h2>
  <label class="setting-row">
    <span class="text">
      <span class="setting-label">{t.settings.releaseNotifications}</span>
      <span class="hint">
        {desktop ? t.settings.releaseNotificationsHint : t.settings.releaseNotificationsHintMobile}
      </span>
    </span>
    <button
      class="setting-switch"
      class:on={ui.settings.releaseNotifications}
      role="switch"
      aria-checked={ui.settings.releaseNotifications}
      aria-label={t.settings.releaseNotifications}
      onclick={() =>
        setUiToggle("releaseNotifications", !ui.settings.releaseNotifications).catch(fail)}
    >
      <span class="knob"></span>
    </button>
  </label>
</div>

<!-- #215: Discord presence, offered only if a Discord application is built in. -->
{#if desktop && discordAvailable}
  <div class="group">
    <h2>{t.settings.discord}</h2>
    <label class="setting-row">
      <span class="text">
        <span class="setting-label">{t.settings.discordPresence}</span>
        <span class="hint">{t.settings.discordHint}</span>
      </span>
      <button
        class="setting-switch"
        class:on={ui.settings.discord}
        role="switch"
        aria-checked={ui.settings.discord}
        aria-label={t.settings.discordPresence}
        onclick={toggleDiscord}
      >
        <span class="knob"></span>
      </button>
    </label>
  </div>
{/if}

<!-- #431: listens sent to ListenBrainz, desktop and phone. -->
<div class="group">
  <h2>{t.settings.listenbrainz.title}</h2>
  <p class="hint">{t.settings.listenbrainz.hint}</p>
  <ListenBrainzSettings />
</div>

<!-- #413: global shortcuts, active outside the window (desktop only). -->
{#if desktop && globalShortcutsAvailable}
  <div class="group global-shortcuts-group">
    <h2>{t.settings.globalShortcuts}</h2>
    <label class="setting-row">
      <span class="text">
        <span class="setting-label">{t.settings.globalShortcutsEnable}</span>
        <span class="hint">{t.settings.globalShortcutsHint}</span>
      </span>
      <button
        class="setting-switch"
        class:on={ui.settings.globalShortcuts.enabled}
        role="switch"
        aria-checked={ui.settings.globalShortcuts.enabled}
        aria-label={t.settings.globalShortcutsEnable}
        onclick={toggleGlobalShortcuts}
      >
        <span class="knob"></span>
      </button>
    </label>
    {#if ui.settings.globalShortcuts.enabled}
      <GlobalShortcutsList />
    {/if}
  </div>
{/if}

<!-- #25: Jam, online through the relay or on the local network (desktop only).
     #452: a switch removes it from the whole app. -->
<div class="group jam">
  <h2>{t.jam.title}</h2>
  <p class="hint">{t.jam.hint}</p>
  <label class="setting-row jam-enable">
    <span class="text">
      <span class="setting-label">{t.jam.enable}</span>
      <span class="hint">{t.jam.enableHint}</span>
    </span>
    <button
      class="setting-switch"
      class:on={ui.settings.jam}
      role="switch"
      aria-checked={ui.settings.jam}
      aria-label={t.jam.enable}
      onclick={() => setJamEnabled(!ui.settings.jam).catch(fail)}
    >
      <span class="knob"></span>
    </button>
  </label>
  {#if ui.settings.jam}
    {#if desktop || jam.onlineAvailable}
      <JamPanel />
    {:else}
      <p class="hint">{t.jam.needsRelay}</p>
    {/if}
  {/if}
</div>

<!-- #336: remote control from a phone, local network, desktop only. -->
{#if desktop}
  <div class="group remote">
    <h2>{t.remote.title}</h2>
    <p class="hint">{t.remote.hint}</p>
    <RemotePanel />
  </div>
{:else}
  <!-- #414: linked computer, to take over or hand back playback (Android). -->
  <div class="group handoff">
    <h2>{t.handoff.title}</h2>
    <p class="hint">{t.handoff.hint}</p>
    <LinkedComputerPanel />
  </div>
{/if}

<!-- #452: relay of the online Jam and of handoffs, built in or chosen. -->
<div class="group relay">
  <h2>{t.jam.relay.title}</h2>
  <p class="hint">{t.jam.relay.hint}</p>
  <RelaySettings />
</div>

<!-- #203: tapped https://tidal.com/… links, to give to the app on Android 12+. -->
{#if links}
  <div class="group">
    <h2>{t.settings.tidalLinks}</h2>
    <div class="setting-row tidal-links">
      <span class="text">
        <span class="setting-label">{t.settings.tidalLinksLabel}</span>
        <span class="hint">
          {links.enabled ? t.settings.tidalLinksOn : t.settings.tidalLinksOff}
        </span>
      </span>
      <button class="update-check" onclick={openLinkSettings}>
        {links.enabled ? t.settings.tidalLinksChange : t.settings.tidalLinksEnable}
      </button>
    </div>
  </div>
{/if}

<style>
  .group {
    margin: 0 10px 28px;
  }

  h2 {
    margin: 0 0 4px;
    font-size: 17px;
  }

  .hint {
    margin: 0 0 14px;
    max-width: 640px;
    font-size: 13px;
    color: var(--text-muted);
  }

  .note {
    margin: 12px 0 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .cache-sizes {
    display: inline-flex;
    gap: 4px;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
  }

  .cache-size {
    min-width: 76px;
    padding: 6px 14px;
    border-radius: var(--radius-pill);
    font-family: var(--font-mono);
    font-size: 13px;
    color: var(--text-muted);
  }

  .cache-size:hover:not(:disabled) {
    color: var(--text);
    background: var(--bg-hover);
  }

  .cache-size.selected {
    color: var(--accent-contrast);
    background: var(--accent);
  }

  .cache-size.selected:hover:not(:disabled) {
    color: var(--accent-contrast);
    background: var(--accent-strong);
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    max-width: 640px;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-raised);
  }

  .setting-row .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .setting-row .setting-label {
    font-weight: 500;
  }

  .setting-row .hint {
    margin: 0;
    font-size: 12px;
  }

  .setting-switch {
    position: relative;
    flex-shrink: 0;
    width: 40px;
    height: 22px;
    border-radius: 999px;
    background: var(--bg-hover);
    box-shadow: inset 0 0 0 1px var(--border);
    transition: background var(--motion-fast);
  }

  .setting-switch .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--text-muted);
    transition:
      transform var(--motion-fast),
      background var(--motion-fast);
  }

  .setting-switch.on {
    background: var(--accent);
  }

  .setting-switch.on .knob {
    transform: translateX(18px);
    background: var(--accent-contrast);
  }

  .setting-switch:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .update-check {
    flex-shrink: 0;
    padding: 6px 14px;
    border: 1px solid var(--border) !important;
    border-radius: 999px;
    font-weight: 600;
    background: var(--control-bg, none);
  }

  .update-check:hover:not(:disabled) {
    background: var(--bg-hover);
  }

  /* Studio: sharp corners. Atelier: barely rounded corners, no background. */
  :global([data-theme="studio"]) .setting-row,
  :global([data-theme="studio"]) .setting-switch,
  :global([data-theme="studio"]) .setting-switch .knob,
  :global([data-theme="studio"]) .update-check {
    border-radius: 0;
  }

  :global([data-theme="atelier"]) .setting-row {
    border-radius: 2px;
  }

  :global([data-theme="atelier"]) .setting-row {
    background: transparent;
  }
</style>
