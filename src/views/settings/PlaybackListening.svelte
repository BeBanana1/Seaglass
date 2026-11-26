<!-- Settings › Playback, second part: normalization, equalizer, end of the queue, content
     filter, blocked items, startup. -->
<script lang="ts">
  import type { EqSettings, Normalization } from "../../lib/ipc";
  import { setContentFilter, ui } from "../../lib/stores/ui.svelte";
  import { t } from "../../lib/i18n.svelte";
  import { platform } from "../../lib/platform.svelte";
  import EqualizerSettings from "../../components/EqualizerSettings.svelte";
  import BlockedList from "../../components/BlockedList.svelte";
  import { fail, type SettingsState } from "./state.svelte";

  let { s }: { s: SettingsState } = $props();

  const desktop = $derived(!platform.mobile);

  const NORMALIZATION_MODES: Normalization[] = ["off", "track", "album"];

  const playback = $derived(s.playback);
  const normalization = $derived(playback?.normalization ?? null);
  /** Exclusive mode (#245): normalization is suspended, its choices grayed out. */
  const exclusive = $derived(playback?.exclusive === true);

  function setNormalization(mode: Normalization) {
    if (playback && mode !== playback.normalization)
      s.savePlayback({ ...playback, normalization: mode });
  }

  function setEqualizer(equalizer: EqSettings) {
    if (playback) s.savePlayback({ ...playback, equalizer });
  }

  function toggleResumeOnLaunch() {
    if (playback) s.savePlayback({ ...playback, resumeOnLaunch: !playback.resumeOnLaunch });
  }

  function toggleAutoRadio() {
    if (playback) s.savePlayback({ ...playback, autoRadio: !playback.autoRadio });
  }

  async function toggleHideExplicit() {
    try {
      await setContentFilter({
        ...ui.settings.content,
        hideExplicit: !ui.settings.content.hideExplicit,
      });
    } catch (e) {
      fail(e);
    }
  }

  async function toggleHideAi() {
    try {
      await setContentFilter({ ...ui.settings.content, hideAi: !ui.settings.content.hideAi });
    } catch (e) {
      fail(e);
    }
  }
</script>

<div class="group">
  <h2>{t.settings.normalization}</h2>
  <p class="hint">{t.settings.normalizationHint}</p>
  <div
    class="cache-sizes normalization-modes"
    role="radiogroup"
    aria-label={t.settings.normalization}
  >
    {#each NORMALIZATION_MODES as mode (mode)}
      <button
        class="cache-size hires-mode normalization-mode"
        class:selected={normalization === mode}
        role="radio"
        aria-checked={normalization === mode}
        disabled={normalization === null || exclusive}
        onclick={() => setNormalization(mode)}
      >
        {t.settings.normalizationModes[mode].label}
      </button>
    {/each}
  </div>
  <p class="note">
    {exclusive
      ? t.settings.normalizationExclusive
      : normalization
        ? t.settings.normalizationModes[normalization].note
        : ""}
  </p>
</div>
<!-- Equalizer (#428, Android #467). -->
{#if playback}
  <div class="group">
    <h2>{t.equalizer.title}</h2>
    <p class="hint">{t.equalizer.hint}</p>
    <EqualizerSettings settings={playback.equalizer} onchange={setEqualizer} />
  </div>
{/if}
<div class="group">
  <h2>{t.settings.queueEnd}</h2>
  <label class="setting-row">
    <span class="text">
      <span class="setting-label">{t.settings.autoRadio}</span>
      <span class="hint">{t.settings.autoRadioHint}</span>
    </span>
    <button
      class="setting-switch"
      class:on={playback?.autoRadio === true}
      role="switch"
      aria-checked={playback?.autoRadio === true}
      aria-label={t.settings.autoRadio}
      disabled={playback === null}
      onclick={toggleAutoRadio}
    >
      <span class="knob"></span>
    </button>
  </label>
</div>
<!-- #368: content filter, applied on the Rust side to the lists loaded afterwards. -->
<div class="group">
  <h2>{t.settings.content}</h2>
  <label class="setting-row">
    <span class="text">
      <span class="setting-label">{t.settings.hideExplicit}</span>
      <span class="hint">{t.settings.hideExplicitHint}</span>
    </span>
    <button
      class="setting-switch"
      class:on={ui.settings.content.hideExplicit}
      role="switch"
      aria-checked={ui.settings.content.hideExplicit}
      aria-label={t.settings.hideExplicit}
      onclick={toggleHideExplicit}
    >
      <span class="knob"></span>
    </button>
  </label>
  <label class="setting-row">
    <span class="text">
      <span class="setting-label">{t.settings.hideAi}</span>
      <span class="hint">{t.settings.hideAiHint}</span>
    </span>
    <button
      class="setting-switch"
      class:on={ui.settings.content.hideAi}
      role="switch"
      aria-checked={ui.settings.content.hideAi}
      aria-label={t.settings.hideAi}
      onclick={toggleHideAi}
    >
      <span class="knob"></span>
    </button>
  </label>
</div>
<!-- #334: blocks saved with the service (a track's menu blocks it). -->
<div class="group">
  <h2>{t.settings.blocked}</h2>
  <p class="hint">{t.settings.blockedHint}</p>
  <BlockedList />
</div>
{#if desktop}
  <div class="group">
    <h2>{t.settings.startup}</h2>
    <label class="setting-row">
      <span class="text">
        <span class="setting-label">{t.settings.resumeOnLaunch}</span>
        <span class="hint">{t.settings.resumeOnLaunchHint}</span>
      </span>
      <button
        class="setting-switch"
        class:on={playback?.resumeOnLaunch === true}
        role="switch"
        aria-checked={playback?.resumeOnLaunch === true}
        aria-label={t.settings.resumeOnLaunch}
        disabled={playback === null}
        onclick={toggleResumeOnLaunch}
      >
        <span class="knob"></span>
      </button>
    </label>
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

  /* Same selectors as the cache (theme styles included), in running text. */
  .hires-mode {
    min-width: 0;
    font-family: inherit;
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

  /* Studio: sharp corners. Atelier: barely rounded corners, no background. */
  :global([data-theme="studio"]) .setting-row,
  :global([data-theme="studio"]) .setting-switch,
  :global([data-theme="studio"]) .setting-switch .knob {
    border-radius: 0;
  }

  :global([data-theme="atelier"]) .setting-row {
    border-radius: 2px;
  }

  :global([data-theme="atelier"]) .setting-row {
    background: transparent;
  }
</style>
