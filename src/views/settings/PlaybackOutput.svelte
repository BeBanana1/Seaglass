<!-- Settings › Playback, first part: output device, exclusive mode, cache and quality on
     desktop; quality per network on Android. -->
<script lang="ts">
  import type { AudioDevice, CacheSize, HiresUpgrade, MobileQuality } from "../../lib/ipc";
  import { t } from "../../lib/i18n.svelte";
  import { platform } from "../../lib/platform.svelte";
  import OutputDevicePicker from "../../components/OutputDevicePicker.svelte";
  import type { SettingsState } from "./state.svelte";

  let { s }: { s: SettingsState } = $props();

  // Android (#24): no audio output or cache (mpv settings), quality chosen among TIDAL's
  // tiers (#366).
  const desktop = $derived(!platform.mobile);

  const CACHE_SIZES: CacheSize[] = [32, 64, 128, 256];

  const HIRES_MODES: HiresUpgrade[] = ["auto", "ask", "off"];

  const MOBILE_QUALITIES: MobileQuality[] = ["low", "high", "lossless", "max"];
  type NetworkQualityKey = "wifiQuality" | "cellularQuality";
  const NETWORKS: NetworkQualityKey[] = ["wifiQuality", "cellularQuality"];

  const playback = $derived(s.playback);
  const cacheSize = $derived(playback?.cacheSize ?? null);
  const hiresUpgrade = $derived(playback?.hiresUpgrade ?? null);
  /** Exclusive mode (#245): normalization is suspended, its choices grayed out. */
  const exclusive = $derived(playback?.exclusive === true);
  const exclusiveAvailable = $derived(s.exclusiveAvailable);
  const qualityCap = $derived(s.qualityCap);

  function setCacheSize(size: CacheSize) {
    if (playback && size !== playback.cacheSize) s.savePlayback({ ...playback, cacheSize: size });
  }

  function setAudioDevice(device: AudioDevice | null) {
    if (playback) s.savePlayback({ ...playback, audioDevice: device });
  }

  function setHiresUpgrade(mode: HiresUpgrade) {
    if (playback && mode !== playback.hiresUpgrade)
      s.savePlayback({ ...playback, hiresUpgrade: mode });
  }

  /** Quality of a network (#365): `wifiQuality` (unmetered) or `cellularQuality`. */
  function setNetworkQuality(network: NetworkQualityKey, quality: MobileQuality) {
    if (playback && quality !== playback[network])
      s.savePlayback({ ...playback, [network]: quality });
  }

  function toggleLowBatteryQuality() {
    if (playback) s.savePlayback({ ...playback, lowBatteryQuality: !playback.lowBatteryQuality });
  }

  function toggleExclusive() {
    if (playback) s.savePlayback({ ...playback, exclusive: !playback.exclusive });
  }

  function toggleDeviceProfiles() {
    if (playback) s.savePlayback({ ...playback, deviceProfiles: !playback.deviceProfiles });
  }

  function togglePauseOnUnplug() {
    if (playback) s.savePlayback({ ...playback, pauseOnUnplug: !playback.pauseOnUnplug });
  }
</script>

{#if desktop}
  <div class="group">
    <h2>{t.settings.outputDevice}</h2>
    <p class="hint">{t.settings.outputDeviceHint}</p>
    <OutputDevicePicker
      value={playback ? playback.audioDevice : undefined}
      onchange={setAudioDevice}
    />
    <label class="setting-row">
      <span class="text">
        <span class="setting-label">{t.settings.pauseOnUnplug}</span>
        <span class="hint">{t.settings.pauseOnUnplugHint}</span>
      </span>
      <button
        class="setting-switch"
        class:on={playback?.pauseOnUnplug === true}
        role="switch"
        aria-checked={playback?.pauseOnUnplug === true}
        aria-label={t.settings.pauseOnUnplug}
        disabled={playback === null}
        onclick={togglePauseOnUnplug}
      >
        <span class="knob"></span>
      </button>
    </label>
    <label class="setting-row">
      <span class="text">
        <span class="setting-label">{t.settings.deviceProfiles}</span>
        <span class="hint">{t.settings.deviceProfilesHint}</span>
      </span>
      <button
        class="setting-switch"
        class:on={playback?.deviceProfiles === true}
        role="switch"
        aria-checked={playback?.deviceProfiles === true}
        aria-label={t.settings.deviceProfiles}
        disabled={playback === null}
        onclick={toggleDeviceProfiles}
      >
        <span class="knob"></span>
      </button>
    </label>
  </div>
  {#if exclusiveAvailable}
    <div class="group">
      <label class="setting-row">
        <span class="text">
          <span class="setting-label">{t.settings.exclusive}</span>
          <span class="hint">{t.settings.exclusiveHint}</span>
        </span>
        <button
          class="setting-switch"
          class:on={exclusive}
          role="switch"
          aria-checked={exclusive}
          aria-label={t.settings.exclusive}
          disabled={playback === null}
          onclick={toggleExclusive}
        >
          <span class="knob"></span>
        </button>
      </label>
    </div>
  {/if}
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
{:else}
  <div class="group">
    <h2>{t.settings.quality}</h2>
    <p class="hint">{t.settings.mobileQualityHint}</p>
    {#each NETWORKS as network (network)}
      <h3 class="network-quality">{t.settings.networks[network]}</h3>
      <div
        class="cache-sizes mobile-qualities"
        role="radiogroup"
        aria-label={t.settings.networks[network]}
      >
        {#each MOBILE_QUALITIES as quality (quality)}
          <button
            class="cache-size hires-mode"
            class:selected={playback?.[network] === quality}
            role="radio"
            aria-checked={playback?.[network] === quality}
            disabled={playback === null}
            onclick={() => setNetworkQuality(network, quality)}
          >
            {t.settings.mobileQualities[quality].label}
          </button>
        {/each}
      </div>
      <p class="note">{playback ? t.settings.mobileQualities[playback[network]].note : ""}</p>
    {/each}
    <label class="setting-row low-battery-quality">
      <span class="text">
        <span class="setting-label">{t.settings.lowBatteryQuality}</span>
        <span class="hint">{t.settings.lowBatteryQualityHint}</span>
      </span>
      <button
        class="setting-switch"
        class:on={playback?.lowBatteryQuality === true}
        role="switch"
        aria-checked={playback?.lowBatteryQuality === true}
        aria-label={t.settings.lowBatteryQuality}
        disabled={playback === null}
        onclick={toggleLowBatteryQuality}
      >
        <span class="knob"></span>
      </button>
    </label>
    {#if qualityCap}
      <p class="note quality-cap">
        {t.settings.qualityCap(
          t.settings.mobileQualities[qualityCap.quality].label,
          t.settings.mobileQualities[qualityCap.chosen].label,
          qualityCap.reason === "powerSave",
          qualityCap.battery,
        )}
      </p>
    {/if}
    <p class="note">{t.settings.mobileQualityNext}</p>
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

  /* Quality per network (#365, Android): subtitles of the same section. */
  .network-quality {
    margin: 18px 0 10px;
    font-size: 14px;
    font-weight: 600;
  }

  .network-quality:first-of-type {
    margin-top: 0;
  }

  .low-battery-quality {
    margin-top: 20px;
  }

  /* Battery cap right now (#492): more visible than an ordinary note. */
  .quality-cap {
    color: var(--warn);
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
