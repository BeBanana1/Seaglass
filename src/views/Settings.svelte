<!-- Page Réglages : une section par onglet. Les valeurs viennent toutes du backend. -->
<script lang="ts">
  import {
    commands,
    type CacheSize,
    type HiresUpgrade,
    type LanguagePref,
    type Normalization,
    type PlaybackSettings,
  } from "../lib/ipc";
  import { replace, type SettingsSection } from "../lib/stores/nav.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { ui } from "../lib/stores/ui.svelte";
  import { setLanguage, t } from "../lib/i18n.svelte";
  import ThemePicker from "../components/ThemePicker.svelte";
  import AccentPicker from "../components/AccentPicker.svelte";

  let { section }: { section: SettingsSection } = $props();

  const sections: SettingsSection[] = ["appearance", "playback", "app"];

  const LANGUAGES: LanguagePref[] = ["system", "fr", "en"];

  const CACHE_SIZES: CacheSize[] = [32, 64, 128, 256];

  const HIRES_MODES: HiresUpgrade[] = ["auto", "ask", "off"];

  const NORMALIZATION_MODES: Normalization[] = ["off", "track", "album"];
  let playback = $state<PlaybackSettings | null>(null);
  const cacheSize = $derived(playback?.cacheSize ?? null);
  const hiresUpgrade = $derived(playback?.hiresUpgrade ?? null);
  const normalization = $derived(playback?.normalization ?? null);
  let autostart = $state<boolean | null>(null);
  let autostartBusy = $state(false);

  $effect(() => {
    if (section === "playback" && playback === null) loadPlayback();
    if (section === "app" && autostart === null) loadAutostart();
  });

  function fail(e: unknown) {
    pushToast({ level: "error", message: String(e) });
  }

  async function loadPlayback() {
    try {
      playback = await commands.playbackSettings();
    } catch (e) {
      fail(e);
    }
  }

  async function savePlayback(next: PlaybackSettings) {
    try {
      await commands.setPlaybackSettings(next);
      playback = next;
    } catch (e) {
      fail(e);
    }
  }

  async function chooseLanguage(pref: LanguagePref) {
    if (pref === ui.settings.language) return;
    try {
      await setLanguage(pref);
      ui.settings = { ...ui.settings, language: pref };
    } catch (e) {
      fail(e);
    }
  }

  function setCacheSize(size: CacheSize) {
    if (playback && size !== playback.cacheSize) savePlayback({ ...playback, cacheSize: size });
  }

  function setNormalization(mode: Normalization) {
    if (playback && mode !== playback.normalization) savePlayback({ ...playback, normalization: mode });
  }

  function setHiresUpgrade(mode: HiresUpgrade) {
    if (playback && mode !== playback.hiresUpgrade) savePlayback({ ...playback, hiresUpgrade: mode });
  }

  async function loadAutostart() {
    try {
      autostart = await commands.autostartEnabled();
    } catch (e) {
      fail(e);
    }
  }

  let updateCheck = $state<"idle" | "checking" | "uptodate" | "ready">("idle");

  async function checkUpdates() {
    if (updateCheck === "checking") return;
    updateCheck = "checking";
    try {
      // Une version prête est réémise en `update-ready` : UpdatePrompt la propose.
      updateCheck = (await commands.checkForUpdates()) ? "ready" : "uptodate";
    } catch (e) {
      updateCheck = "idle";
      fail(t.settings.checkFailed(String(e)));
    }
  }

  async function toggleAutostart() {
    if (autostart === null || autostartBusy) return;
    autostartBusy = true;
    try {
      autostart = await commands.setAutostart(!autostart);
    } catch (e) {
      fail(e);
      await loadAutostart();
    } finally {
      autostartBusy = false;
    }
  }
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
      <ThemePicker />
    </div>
    <div class="group">
      <h2>{t.settings.accent}</h2>
      <p class="hint">{t.settings.accentHint}</p>
      <AccentPicker />
    </div>
  {:else if section === "playback"}
    <div class="group">
      <h2>{t.settings.cache}</h2>
      <p class="hint">{t.settings.cacheHint}</p>
      <div class="cache-sizes" role="radiogroup" aria-label={t.settings.cacheSize}>
        {#each CACHE_SIZES as size (size)}
          <button
            class="cache-size"
            class:selected={cacheSize === size}
            role="radio"
            aria-checked={cacheSize === size}
            disabled={cacheSize === null}
            onclick={() => setCacheSize(size)}
          >
            {t.settings.megabytes(size)}
          </button>
        {/each}
      </div>
      <p class="note">{t.settings.cacheNote}</p>
    </div>
    <div class="group">
      <h2>{t.settings.quality}</h2>
      <p class="hint">{t.settings.qualityHint}</p>
      <div class="cache-sizes hires-modes" role="radiogroup" aria-label={t.settings.hiresEdition}>
        {#each HIRES_MODES as mode (mode)}
          <button
            class="cache-size hires-mode"
            class:selected={hiresUpgrade === mode}
            role="radio"
            aria-checked={hiresUpgrade === mode}
            disabled={hiresUpgrade === null}
            onclick={() => setHiresUpgrade(mode)}
          >
            {t.settings.hiresModes[mode].label}
          </button>
        {/each}
      </div>
      <p class="note">{hiresUpgrade ? t.settings.hiresModes[hiresUpgrade].note : ""}</p>
    </div>
    <div class="group">
      <h2>{t.settings.normalization}</h2>
      <p class="hint">{t.settings.normalizationHint}</p>
      <!-- Le mode exclusif / bit-perfect (phase 5) devra griser ces choix. -->
      <div class="cache-sizes normalization-modes" role="radiogroup" aria-label={t.settings.normalization}>
        {#each NORMALIZATION_MODES as mode (mode)}
          <button
            class="cache-size hires-mode normalization-mode"
            class:selected={normalization === mode}
            role="radio"
            aria-checked={normalization === mode}
            disabled={normalization === null}
            onclick={() => setNormalization(mode)}
          >
            {t.settings.normalizationModes[mode].label}
          </button>
        {/each}
      </div>
      <p class="note">{normalization ? t.settings.normalizationModes[normalization].note : ""}</p>
    </div>
  {:else}
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
          onclick={toggleAutostart}
        >
          <span class="knob"></span>
        </button>
      </label>
    </div>

    <div class="group">
      <h2>{t.settings.updates}</h2>
      <div class="setting-row">
        <span class="text">
          <span class="setting-label">{t.settings.checkUpdates}</span>
          <span class="hint">
            {#if updateCheck === "uptodate"}
              {t.settings.upToDate}
            {:else if updateCheck === "ready"}
              {t.settings.updateReady}
            {:else}
              {t.settings.updateAuto}
            {/if}
          </span>
        </span>
        <button class="update-check" onclick={checkUpdates} disabled={updateCheck === "checking"}>
          {updateCheck === "checking" ? t.settings.checking : t.settings.check}
        </button>
      </div>
    </div>

    <div class="group about">
      <h2>{t.settings.about}</h2>
      <p><span class="app-name">Seaglass</span> <span class="mono">v{__APP_VERSION__}</span></p>
      <p><strong>{t.settings.disclaimer}</strong></p>
      <p>{t.settings.streamingOnly}</p>
      <p>{t.settings.license}</p>
    </div>
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

  /* Mêmes sélecteurs que le cache (styles des thèmes compris), en texte courant. */
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

  .about p {
    margin: 6px 0;
    color: var(--text-muted);
  }

  .about strong,
  .app-name {
    font-weight: 600;
    color: var(--text);
  }

  .mono {
    font-family: var(--font-mono);
    font-size: 12px;
  }

  /* Studio : angles vifs. Atelier : angles à peine arrondis, pas de fond. */
  :global([data-theme="studio"]) .tabs button,
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
