<!-- Settings › Application, last part: updates and changelog, about, account. -->
<script lang="ts">
  import { commands, type ProjectPage } from "../../lib/ipc";
  import { setUiToggle, ui } from "../../lib/stores/ui.svelte";
  import { t } from "../../lib/i18n.svelte";
  import { platform } from "../../lib/platform.svelte";
  import ChangelogList from "../../components/ChangelogList.svelte";
  import CreditsRoll from "../../components/CreditsRoll.svelte";
  import { fail, type SettingsState } from "./state.svelte";

  let { s, onLogout }: { s: SettingsState; onLogout?: () => void } = $props();

  // Android (#24): no desktop updater, so "Check" does not depend on `updatesAvailable`.
  const desktop = $derived(!platform.mobile);

  const updatesAvailable = $derived(s.updatesAvailable);
  const updateServer = $derived(s.updateServer);
  const updateCheck = $derived(s.updateCheck);
  const downloadingVersion = $derived(s.downloadingVersion);

  async function openProject(page: ProjectPage) {
    try {
      await commands.openProjectPage(page);
    } catch (e) {
      fail(e);
    }
  }
</script>

<div class="group">
  <h2>{t.settings.updates}</h2>
  <div class="setting-row">
    <span class="text">
      <span class="setting-label">{t.settings.changelog}</span>
      <span class="hint">{t.settings.changelogHint}</span>
    </span>
    <button
      class="update-check"
      onclick={() => (s.changelogOpen = !s.changelogOpen)}
      aria-expanded={s.changelogOpen}
    >
      {s.changelogOpen ? t.settings.hideChangelog : t.settings.showChangelog}
    </button>
  </div>
  {#if s.changelogOpen}<ChangelogList />{/if}
  <!-- #452: nothing to check without a built-in server; periodic check can be turned off. -->
  {#if updateServer !== null}
    {#if !desktop || updatesAvailable}
      <label class="setting-row">
        <span class="text">
          <span class="setting-label">{t.settings.autoUpdates}</span>
          <span class="hint">{t.settings.autoUpdatesHint(updateServer)}</span>
        </span>
        <button
          class="setting-switch"
          class:on={ui.settings.autoUpdates}
          role="switch"
          aria-checked={ui.settings.autoUpdates}
          aria-label={t.settings.autoUpdates}
          onclick={() => setUiToggle("autoUpdates", !ui.settings.autoUpdates).catch(fail)}
        >
          <span class="knob"></span>
        </button>
      </label>
    {/if}
    <div class="setting-row">
      <span class="text">
        <span class="setting-label">{t.settings.checkUpdates}</span>
        <span class="hint">
          {#if desktop && !updatesAvailable}
            {t.settings.updatesManual}
          {:else if updateCheck === "uptodate"}
            {t.settings.upToDate}
          {:else if updateCheck === "downloading"}
            {t.settings.updateDownloading(downloadingVersion)}
          {:else if updateCheck === "ready"}
            {desktop ? t.settings.updateReady : t.settings.updateAvailable}
          {:else}
            {ui.settings.autoUpdates ? t.settings.updateAuto : t.settings.updateManualOnly}
          {/if}
        </span>
      </span>
      {#if !desktop || updatesAvailable}
        <button
          class="update-check"
          onclick={s.checkUpdates}
          disabled={updateCheck === "checking" || updateCheck === "downloading"}
        >
          {updateCheck === "checking" ? t.settings.checking : t.settings.check}
        </button>
      {/if}
    </div>
  {/if}
</div>
<div class="group about">
  <h2>{t.settings.about}</h2>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <p>
    <span class="app-name">Seaglass</span>
    <span class="mono" onclick={s.clickVersion}>v{__APP_VERSION__}</span>
  </p>
  {#if s.creditsOpen}<CreditsRoll onClose={() => (s.creditsOpen = false)} />{/if}
  <p><strong>{t.settings.disclaimer}</strong></p>
  <p>{t.settings.clientNotice}</p>
  <p>{t.settings.streamingOnly}</p>
  <p>{t.settings.license}</p>
  <!-- #188: fixed URLs on the Rust side, opened in the system browser. -->
  <div class="project-links">
    <button class="update-check" onclick={() => openProject("website")}>
      {t.settings.website}
    </button>
    <button class="update-check" onclick={() => openProject("clientId")}>
      {t.settings.clientNoticeMore}
    </button>
    <button class="update-check" onclick={() => openProject("repo")}>
      {t.settings.sourceCode}
    </button>
    <button
      class="update-check"
      onclick={() => openProject("newIssue")}
      title={t.settings.reportIssueHint}
    >
      {t.settings.reportIssue}
    </button>
  </div>
</div>

<!-- #189: under About, on both platforms (no longer in the desktop header). -->
{#if onLogout}
  <div class="group">
    <h2>{t.settings.account}</h2>
    <button class="update-check logout" onclick={onLogout}>{t.common.logout}</button>
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

  .update-check:disabled {
    opacity: 0.6;
    cursor: default;
  }

  /* #189: destructive action in red, each theme's red (`--danger`). `:root` comes before the
     `.update-check` overrides of the theme files; the background stays theirs. */
  :global(:root) .update-check.logout {
    color: var(--danger);
    border-color: var(--danger) !important;
  }

  :global(:root) .update-check.logout:hover {
    background: color-mix(in srgb, var(--danger) 14%, transparent);
  }

  .about p {
    margin: 6px 0;
    color: var(--text-muted);
  }

  .about strong,
  .app-name {
    font-weight: 600;
    color: var(--text);
  }

  .project-links {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 14px;
  }

  .mono {
    font-family: var(--font-mono);
    font-size: 12px;
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
