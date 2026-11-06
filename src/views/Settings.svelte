<!-- Page Réglages : une section par onglet. Les valeurs viennent toutes du backend. -->
<script lang="ts">
  import {
    commands,
    events,
    type AudioDevice,
    type CacheSize,
    type CloseAction,
    type HiresUpgrade,
    type JamMode,
    type LanguagePref,
    type Normalization,
    type PlaybackSettings,
  } from "../lib/ipc";
  import { replace, type SettingsSection } from "../lib/stores/nav.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { setCloseAction, ui } from "../lib/stores/ui.svelte";
  import { setLanguage, t } from "../lib/i18n.svelte";
  import ThemePicker from "../components/ThemePicker.svelte";
  import AccentPicker from "../components/AccentPicker.svelte";
  import OutputDevicePicker from "../components/OutputDevicePicker.svelte";
  import { platform } from "../lib/platform.svelte";
  import { jam, startJam, stopJam } from "../lib/stores/jam.svelte";

  let { section, onLogout }: { section: SettingsSection; onLogout?: () => void } = $props();

  // Android (#24) : ni sortie audio ni cache (réglages de mpv), qualité fixée à Lossless (pas
  // d'édition hi-res), ni fenêtre, démarrage auto ou mises à jour (réglages du bureau).
  const desktop = $derived(!platform.mobile);

  const sections: SettingsSection[] = ["appearance", "playback", "app"];

  const LANGUAGES: LanguagePref[] = ["system", "fr", "en"];

  const CLOSE_ACTIONS: CloseAction[] = ["quit", "tray"];

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
    if (section === "app" && autostart === null && desktop) loadAutostart();
  });

  async function launchJam(mode: JamMode) {
    try {
      await startJam(mode);
    } catch (e) {
      fail(e);
    }
  }

  async function endJam() {
    try {
      await stopJam();
      pushToast({ level: "info", message: t.jam.stopped });
    } catch (e) {
      fail(e);
    }
  }

  async function copyJamUrl() {
    if (!jam.status?.url) return;
    try {
      await navigator.clipboard.writeText(jam.status.url);
      pushToast({ level: "info", message: t.jam.copied });
    } catch (e) {
      fail(e);
    }
  }

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

  async function chooseCloseAction(action: CloseAction) {
    if (action === ui.settings.closeAction) return;
    try {
      await setCloseAction(action);
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

  function setAudioDevice(device: AudioDevice | null) {
    if (playback) savePlayback({ ...playback, audioDevice: device });
  }

  function setHiresUpgrade(mode: HiresUpgrade) {
    if (playback && mode !== playback.hiresUpgrade) savePlayback({ ...playback, hiresUpgrade: mode });
  }

  function toggleAutoRadio() {
    if (playback) savePlayback({ ...playback, autoRadio: !playback.autoRadio });
  }

  async function loadAutostart() {
    try {
      autostart = await commands.autostartEnabled();
    } catch (e) {
      fail(e);
    }
  }

  let updateCheck = $state<"idle" | "checking" | "uptodate" | "downloading" | "ready">("idle");
  let downloadingVersion = $state("");

  // Bureau (#187) : la vérification répond avant la fin du téléchargement de l'installeur.
  $effect(() => {
    const ready = events.onUpdateReady(() => {
      if (updateCheck === "downloading") updateCheck = "ready";
    });
    const failed = events.onUpdateFailed((message) => {
      if (updateCheck !== "downloading") return;
      updateCheck = "idle";
      fail(t.settings.checkFailed(message));
    });
    return () => {
      void ready.then((f) => f());
      void failed.then((f) => f());
    };
  });

  async function checkUpdates() {
    if (updateCheck === "checking" || updateCheck === "downloading") return;
    updateCheck = "checking";
    try {
      // Une version prête est réémise en `update-ready` : UpdatePrompt la propose.
      const info = await commands.checkForUpdates();
      if (info?.downloading) downloadingVersion = info.version;
      updateCheck = !info ? "uptodate" : info.downloading ? "downloading" : "ready";
    } catch (e) {
      updateCheck = "idle";
      fail(t.settings.checkFailed(String(e)));
    }
  }

  async function clearPlayHistory() {
    try {
      await commands.clearPlayHistory();
      pushToast({ level: "info", message: t.settings.playHistoryCleared });
    } catch (e) {
      fail(e);
    }
  }

  async function openProject(page: "repo" | "newIssue") {
    try {
      await commands.openProjectPage(page);
    } catch (e) {
      fail(e);
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
    {#if desktop}
    <div class="group">
      <h2>{t.settings.outputDevice}</h2>
      <p class="hint">{t.settings.outputDeviceHint}</p>
      <OutputDevicePicker value={playback ? playback.audioDevice : undefined} onchange={setAudioDevice} />
    </div>
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
    {/if}
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
  {:else}
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
          onclick={toggleAutostart}
        >
          <span class="knob"></span>
        </button>
      </label>
    </div>

    <!-- #25 : Jam locale, page web servie sur le réseau local. -->
    <div class="group jam">
      <h2>{t.jam.title}</h2>
      <p class="hint">{t.jam.hint}</p>
      {#if jam.status}
        <p
          class="jam-connection"
          class:warn={jam.status.connection === "reconnecting" || jam.status.connection === "failed"}
          role="status"
        >
          {jam.status.mode === "local" ? t.jam.connection.local : t.jam.connection[jam.status.connection]}
          {#if jam.status.reason}<span class="jam-reason">({jam.status.reason})</span>{/if}
        </p>
        <div class="setting-row jam-info">
          {#if jam.status.qrSvg}
            <!-- SVG produit par le backend (crate qrcode) à partir de l'adresse et du code. -->
            <div class="jam-qr" role="img" aria-label={t.jam.qr}>{@html jam.status.qrSvg}</div>
          {/if}
          <dl class="jam-fields">
            <dt>{t.jam.address}</dt>
            <dd class="mono jam-url">{jam.status.url ?? "…"}</dd>
            <dt>{t.jam.code}</dt>
            <dd class="mono jam-code">{jam.status.code}</dd>
          </dl>
          <div class="jam-actions">
            <button class="update-check" onclick={copyJamUrl} disabled={!jam.status.url}>{t.jam.copy}</button>
            <button class="update-check" onclick={endJam}>{t.jam.stop}</button>
          </div>
        </div>
        <p class="note">{jam.status.lan ? t.jam.howTo : t.jam.noLan}</p>
      {:else}
        <!-- En ligne par défaut (relais, sans Wi-Fi commun) ; le réseau local en second choix. -->
        <div class="setting-row">
          <span class="text">
            <span class="hint">{jam.onlineAvailable ? t.jam.onlineHint : t.jam.localHint}</span>
          </span>
          <div class="jam-actions">
            {#if jam.onlineAvailable}
              <button class="update-check" onclick={() => launchJam("online")} disabled={jam.busy}>
                {jam.busy ? t.jam.starting : t.jam.start}
              </button>
              <button class="update-check" onclick={() => launchJam("local")} disabled={jam.busy}>
                {t.jam.startLocal}
              </button>
            {:else}
              <button class="update-check" onclick={() => launchJam("local")} disabled={jam.busy}>
                {jam.busy ? t.jam.starting : t.jam.start}
              </button>
            {/if}
          </div>
        </div>
        <p class="note">{t.jam.localHint} {t.jam.firewall}</p>
      {/if}
    </div>
    {/if}

    <!-- #202 : écoutes gardées localement (accueil, « À la une »). -->
    <div class="group">
      <h2>{t.settings.playHistory}</h2>
      <div class="setting-row">
        <span class="text">
          <span class="hint">{t.settings.playHistoryHint}</span>
        </span>
        <button class="update-check" onclick={clearPlayHistory}>{t.settings.clearPlayHistory}</button>
      </div>
    </div>

    <div class="group">
      <h2>{t.settings.updates}</h2>
      <div class="setting-row">
        <span class="text">
          <span class="setting-label">{t.settings.checkUpdates}</span>
          <span class="hint">
            {#if updateCheck === "uptodate"}
              {t.settings.upToDate}
            {:else if updateCheck === "downloading"}
              {t.settings.updateDownloading(downloadingVersion)}
            {:else if updateCheck === "ready"}
              {desktop ? t.settings.updateReady : t.settings.updateAvailable}
            {:else}
              {t.settings.updateAuto}
            {/if}
          </span>
        </span>
        <button class="update-check" onclick={checkUpdates} disabled={updateCheck === "checking" || updateCheck === "downloading"}>
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
      <!-- #188 : URLs fixes côté Rust, ouvertes dans le navigateur du système. -->
      <div class="project-links">
        <button class="update-check" onclick={() => openProject("repo")}>{t.settings.sourceCode}</button>
        <button class="update-check" onclick={() => openProject("newIssue")} title={t.settings.reportIssueHint}>
          {t.settings.reportIssue}
        </button>
      </div>
    </div>

    <!-- #189 : sous À propos, sur les deux plateformes (plus dans l'en-tête du bureau). -->
    {#if onLogout}
    <div class="group">
      <h2>{t.settings.account}</h2>
      <button class="update-check logout" onclick={onLogout}>{t.common.logout}</button>
    </div>
    {/if}
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

  /* #189 : action destructrice en rouge, au rouge de chaque thème (`--danger`). `:root` passe
     devant les surcharges de `.update-check` des fichiers de thème ; le fond reste le leur. */
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

  .setting-row.jam-info {
    flex-wrap: wrap;
    gap: 16px;
  }

  .jam-connection {
    margin: 0 0 10px;
    font-size: 13px;
    font-weight: 600;
    color: var(--accent);
  }

  .jam-connection.warn {
    color: var(--warn, var(--text-muted));
  }

  .jam-reason {
    font-weight: 400;
    color: var(--text-muted);
  }

  .jam-qr {
    flex: none;
    width: 148px;
    height: 148px;
    padding: 6px;
    border-radius: 10px;
    background: #ffffff;
  }

  .jam-qr :global(svg) {
    display: block;
    width: 100%;
    height: 100%;
  }

  .jam-fields {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 16px;
    align-items: baseline;
    margin: 0;
    flex: 1;
    min-width: 220px;
  }

  .jam-fields dt {
    font-size: 13px;
    color: var(--text-muted);
  }

  .jam-fields dd {
    margin: 0;
    overflow-wrap: anywhere;
    user-select: text;
  }

  .jam-code {
    font-size: 22px;
    letter-spacing: 0.2em;
  }

  .jam-actions {
    display: flex;
    gap: 8px;
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
