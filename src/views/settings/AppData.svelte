<!-- Settings › Application, second part: play history, logs, library export, settings
     backup. -->
<script lang="ts">
  import { commands } from "../../lib/ipc";
  import { t } from "../../lib/i18n.svelte";
  import { copyText, pushToast } from "../../lib/stores/toasts.svelte";
  import {
    cancelLibraryExport,
    libraryExport,
    startLibraryExport,
  } from "../../lib/stores/libraryExport.svelte";
  import { platform } from "../../lib/platform.svelte";
  import { fail, type SettingsState } from "./state.svelte";

  let { s }: { s: SettingsState } = $props();

  const desktop = $derived(!platform.mobile);

  const importing = $derived(s.importing);

  async function clearPlayHistory() {
    try {
      await commands.clearPlayHistory();
      pushToast({ level: "info", message: t.settings.playHistoryCleared });
    } catch (e) {
      fail(e);
    }
  }

  function copyLog() {
    void copyText(commands.currentLog, t.settings.logCopied, t.settings.logUnavailable);
  }

  function copySettings() {
    void copyText(commands.exportSettings, t.settings.settingsCopied, t.settings.logUnavailable);
  }

  function copyCrash() {
    void copyText(commands.lastCrashReport, t.settings.crashCopied, t.settings.noCrash);
  }
</script>

<!-- #202: listens kept locally (home, "Featured"). -->
<div class="group">
  <h2>{t.settings.playHistory}</h2>
  <div class="setting-row">
    <span class="text">
      <span class="hint">{t.settings.playHistoryHint}</span>
    </span>
    <button class="update-check" onclick={clearPlayHistory}>
      {t.settings.clearPlayHistory}
    </button>
  </div>
</div>

<!-- #192: log and crash report kept locally, copied for an issue. -->
<div class="group">
  <h2>{t.settings.logs}</h2>
  <p class="hint">{t.settings.logsHint}</p>
  <div class="project-links">
    <button class="update-check" onclick={copyLog}>{t.settings.copyLog}</button>
    <button class="update-check" onclick={copyCrash}>{t.settings.copyCrash}</button>
    {#if desktop}
      <button class="update-check" onclick={() => commands.openLogDir().catch(fail)}>
        {t.settings.openLogDir}
      </button>
    {/if}
  </div>
</div>

<!-- #330: library exported as JSON and CSV (desktop: Documents folder). -->
{#if desktop}
  <div class="group">
    <h2>{t.settings.libraryExport}</h2>
    <p class="hint">{t.settings.libraryExportHint}</p>
    <div class="project-links">
      {#if libraryExport.running}
        <span class="hint">
          {t.settings.exportProgress(libraryExport.done, libraryExport.total)}
        </span>
        <button class="update-check" onclick={cancelLibraryExport}>{t.common.cancel}</button>
      {:else}
        <button class="update-check" onclick={startLibraryExport}>
          {t.settings.exportLibrary}
        </button>
      {/if}
    </div>
  </div>
{/if}

<!-- #352: settings copied as JSON, pasted on another device. -->
<div class="group">
  <h2>{t.settings.backup}</h2>
  <p class="hint">{t.settings.backupHint}</p>
  <div class="project-links">
    <button class="update-check" onclick={copySettings}>{t.settings.copySettings}</button>
    {#if s.importText === null}
      <button class="update-check" onclick={() => (s.importText = "")}>
        {t.settings.importSettings}
      </button>
    {/if}
  </div>
  {#if s.importText !== null}
    <textarea
      class="import-text mono"
      rows="6"
      spellcheck="false"
      placeholder={t.settings.importPlaceholder}
      bind:value={s.importText}></textarea>
    <div class="project-links">
      <button
        class="update-check"
        onclick={s.applyImport}
        disabled={importing || !s.importText.trim()}
      >
        {t.settings.applyImport}
      </button>
      <button class="update-check" onclick={() => (s.importText = null)}>
        {t.common.cancel}
      </button>
    </div>
  {/if}
</div>

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

  .setting-row .hint {
    margin: 0;
    font-size: 12px;
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

  .import-text {
    display: block;
    width: 100%;
    max-width: 640px;
    margin: 12px 0;
    padding: 8px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    color: var(--text);
    background: var(--bg-raised);
    font-size: 12px;
    resize: vertical;
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
