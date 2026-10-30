<!-- Page Réglages : une section par onglet. Les valeurs viennent toutes du backend. -->
<script lang="ts">
  import { commands, type CacheSize } from "../lib/ipc";
  import { replace, type SettingsSection } from "../lib/stores/nav.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import ThemePicker from "../components/ThemePicker.svelte";

  let { section }: { section: SettingsSection } = $props();

  const sections: { id: SettingsSection; label: string }[] = [
    { id: "appearance", label: "Apparence" },
    { id: "playback", label: "Lecture" },
    { id: "app", label: "Application" },
  ];

  const CACHE_SIZES: CacheSize[] = [32, 64, 128, 256];

  let cacheSize = $state<CacheSize | null>(null);
  let autostart = $state<boolean | null>(null);
  let autostartBusy = $state(false);

  $effect(() => {
    if (section === "playback" && cacheSize === null) loadPlayback();
    if (section === "app" && autostart === null) loadAutostart();
  });

  function fail(e: unknown) {
    pushToast({ level: "error", message: String(e) });
  }

  async function loadPlayback() {
    try {
      cacheSize = (await commands.playbackSettings()).cacheSize;
    } catch (e) {
      fail(e);
    }
  }

  async function setCacheSize(size: CacheSize) {
    if (size === cacheSize) return;
    try {
      await commands.setPlaybackSettings({ cacheSize: size });
      cacheSize = size;
    } catch (e) {
      fail(e);
    }
  }

  async function loadAutostart() {
    try {
      autostart = await commands.autostartEnabled();
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
  <h1>Réglages</h1>
  <div class="tabs" role="tablist">
    {#each sections as s (s.id)}
      <button
        role="tab"
        aria-selected={s.id === section}
        class:active={s.id === section}
        onclick={() => replace({ name: "settings", section: s.id })}
      >
        {s.label}
      </button>
    {/each}
  </div>

  {#if section === "appearance"}
    <div class="group">
      <h2>Thème</h2>
      <p class="hint">Le thème s'applique tout de suite et reste enregistré.</p>
      <ThemePicker />
    </div>
  {:else if section === "playback"}
    <div class="group">
      <h2>Cache de lecture</h2>
      <p class="hint">
        Mémoire réservée à la lecture en avance de chaque titre chargé (en cours et suivant).
        64 Mo couvrent environ 5 minutes de FLAC 24 bits / 48 kHz, une dizaine en 16 bits /
        44,1 kHz. Un cache plus grand résiste mieux à une connexion instable.
      </p>
      <div class="cache-sizes" role="radiogroup" aria-label="Taille du cache">
        {#each CACHE_SIZES as size (size)}
          <button
            class="cache-size"
            class:selected={cacheSize === size}
            role="radio"
            aria-checked={cacheSize === size}
            disabled={cacheSize === null}
            onclick={() => setCacheSize(size)}
          >
            {size} Mo
          </button>
        {/each}
      </div>
      <p class="note">
        Appliqué tout de suite, sans interrompre la lecture. Le cache reste en mémoire : rien
        n'est écrit sur disque.
      </p>
    </div>
  {:else}
    <div class="group">
      <h2>Démarrage</h2>
      <label class="setting-row">
        <span class="text">
          <span class="setting-label">Lancer Seaglass à l'ouverture de session</span>
          <span class="hint">Réglage de Windows : il peut aussi être changé dans le Gestionnaire des tâches.</span>
        </span>
        <button
          class="setting-switch"
          class:on={autostart === true}
          role="switch"
          aria-checked={autostart === true}
          aria-label="Lancer Seaglass à l'ouverture de session"
          disabled={autostart === null || autostartBusy}
          onclick={toggleAutostart}
        >
          <span class="knob"></span>
        </button>
      </label>
    </div>

    <div class="group about">
      <h2>À propos</h2>
      <p><span class="app-name">Seaglass</span> <span class="mono">v{__APP_VERSION__}</span></p>
      <p><strong>Client non officiel, non affilié à TIDAL.</strong></p>
      <p>Streaming uniquement : aucun flux n'est téléchargé ni enregistré sur disque.</p>
      <p>Licence GPL-3.0. Lecture assurée par mpv.</p>
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
    border-radius: 999px;
  }

  .cache-size {
    min-width: 76px;
    padding: 6px 14px;
    border-radius: 999px;
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
    transition: background 0.15s;
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
      transform 0.15s,
      background 0.15s;
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
  :global([data-theme="studio"]) .cache-sizes,
  :global([data-theme="studio"]) .cache-size,
  :global([data-theme="studio"]) .setting-row,
  :global([data-theme="studio"]) .setting-switch,
  :global([data-theme="studio"]) .setting-switch .knob {
    border-radius: 0;
  }

  :global([data-theme="atelier"]) .cache-sizes,
  :global([data-theme="atelier"]) .cache-size,
  :global([data-theme="atelier"]) .setting-row {
    border-radius: 2px;
  }

  :global([data-theme="atelier"]) .setting-row {
    background: transparent;
  }
</style>
