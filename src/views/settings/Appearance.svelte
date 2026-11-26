<!-- Settings › Appearance: language, theme, motion, easter eggs, zoom, theme gallery. -->
<script lang="ts">
  import type { LanguagePref } from "../../lib/ipc";
  import {
    MOTIONS,
    setEasterEggs,
    setMotion,
    setZoom,
    ui,
    ZOOM_LEVELS,
  } from "../../lib/stores/ui.svelte";
  import { setLanguage, t } from "../../lib/i18n.svelte";
  import { platform } from "../../lib/platform.svelte";
  import ThemePicker from "../../components/ThemePicker.svelte";
  import CurrentTheme from "../../components/CurrentTheme.svelte";
  import { fail } from "./state.svelte";

  const desktop = $derived(!platform.mobile);

  const LANGUAGES: LanguagePref[] = ["system", "fr", "en"];

  async function chooseLanguage(pref: LanguagePref) {
    if (pref === ui.settings.language) return;
    try {
      await setLanguage(pref);
      ui.settings = { ...ui.settings, language: pref };
    } catch (e) {
      fail(e);
    }
  }
</script>

<div class="group">
  <h2>{t.settings.language}</h2>
  <p class="hint">{t.settings.languageHint}</p>
  <div class="cache-sizes languages" role="radiogroup" aria-label={t.settings.language}>
    {#each LANGUAGES as lang (lang)}
      <button
        class="cache-size language"
        class:selected={ui.settings.language === lang}
        role="radio"
        aria-checked={ui.settings.language === lang}
        lang={lang === "system" ? undefined : lang}
        onclick={() => chooseLanguage(lang)}
      >
        {t.settings.languages[lang]}
      </button>
    {/each}
  </div>
</div>
<div class="group">
  <h2>{t.settings.theme}</h2>
  <p class="hint">{t.settings.themeHint}</p>
  <CurrentTheme />
</div>
<div class="group">
  <h2>{t.settings.motion}</h2>
  <p class="hint">{t.settings.motionHint}</p>
  <div class="cache-sizes motions" role="radiogroup" aria-label={t.settings.motion}>
    {#each MOTIONS as motion (motion)}
      <button
        class="cache-size motion"
        class:selected={ui.settings.motion === motion}
        role="radio"
        aria-checked={ui.settings.motion === motion}
        onclick={() => setMotion(motion).catch(fail)}
      >
        {t.settings.motions[motion]}
      </button>
    {/each}
  </div>
</div>
<!-- #399: easter eggs (dates, searches), on by default. -->
<div class="group">
  <label class="setting-row">
    <span class="text">
      <span class="setting-label">{t.settings.eggs}</span>
      <span class="hint">{t.settings.eggsHint}</span>
    </span>
    <button
      class="setting-switch"
      class:on={ui.settings.easterEggs}
      role="switch"
      aria-checked={ui.settings.easterEggs}
      aria-label={t.settings.eggs}
      onclick={() => setEasterEggs(!ui.settings.easterEggs).catch(fail)}
    >
      <span class="knob"></span>
    </button>
  </label>
</div>
{#if desktop}
  <div class="group">
    <h2>{t.settings.zoom}</h2>
    <p class="hint">{t.settings.zoomHint}</p>
    <div class="cache-sizes" role="radiogroup" aria-label={t.settings.zoom}>
      {#each ZOOM_LEVELS as level (level)}
        <button
          class="cache-size"
          class:selected={ui.settings.zoom === level}
          role="radio"
          aria-checked={ui.settings.zoom === level}
          onclick={() => setZoom(level).catch(fail)}
        >
          {level} %
        </button>
      {/each}
    </div>
  </div>
{/if}
<!-- Gallery last (#390): no setting hidden below it anymore. -->
<div class="group">
  <h2>{t.settings.themeGallery}</h2>
  <ThemePicker />
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
