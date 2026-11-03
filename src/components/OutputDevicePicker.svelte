<!-- Réglages › Lecture › Sortie audio (#79) : périphérique par défaut du système ou un
     périphérique WASAPI. La liste vient de mpv et suit les branchements. -->
<script lang="ts">
  import { commands, events, type AudioDevice, type AudioDevices } from "../lib/ipc";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { t } from "../lib/i18n.svelte";

  let {
    value,
    disabled = false,
    onchange,
  }: {
    /** Choix enregistré ; `undefined` tant que les réglages ne sont pas chargés. */
    value: AudioDevice | null | undefined;
    disabled?: boolean;
    onchange: (device: AudioDevice | null) => void;
  } = $props();

  let known = $state<AudioDevices | null>(null);

  $effect(() => {
    let unlisten: (() => void) | undefined;
    let alive = true;
    commands
      .audioDevices()
      .then((d) => {
        if (alive) known ??= d;
      })
      .catch((e) => pushToast({ level: "error", message: String(e) }));
    events
      .onAudioDevices((d) => (known = d))
      .then((u) => {
        if (alive) unlisten = u;
        else u();
      });
    return () => {
      alive = false;
      unlisten?.();
    };
  });

  const defaultName = $derived(
    known?.systemDefault ? (known.devices.find((d) => sameId(d.id, known!.systemDefault!))?.name ?? null) : null,
  );

  /** Choix enregistré absent de la liste (débranché) : affiché quand même, sélectionné. */
  const missing = $derived(
    value && known && !known.devices.some((d) => sameId(d.id, value.id)) ? value : null,
  );

  function sameId(a: string, b: string) {
    return a.toLowerCase() === b.toLowerCase();
  }

  function isSelected(device: AudioDevice | null) {
    if (value === undefined) return false;
    return device === null ? value === null : value !== null && sameId(value.id, device.id);
  }

  function choose(device: AudioDevice | null) {
    if (!isSelected(device)) onchange(device ? { id: device.id, name: device.name } : null);
  }
</script>

<div class="cache-sizes audio-devices" role="radiogroup" aria-label={t.settings.outputDevice}>
  <button
    class="cache-size audio-device"
    class:selected={isSelected(null)}
    role="radio"
    aria-checked={isSelected(null)}
    disabled={disabled || value === undefined}
    onclick={() => choose(null)}
  >
    <span class="device-name">{t.settings.systemDefault}</span>
    {#if defaultName}<span class="device-detail">{defaultName}</span>{/if}
  </button>
  {#each known?.devices ?? [] as device (device.id)}
    <button
      class="cache-size audio-device"
      class:selected={isSelected(device)}
      role="radio"
      aria-checked={isSelected(device)}
      disabled={disabled || value === undefined}
      onclick={() => choose(device)}
    >
      <span class="device-name">{device.name}</span>
    </button>
  {/each}
  {#if missing}
    <button class="cache-size audio-device selected missing" role="radio" aria-checked="true" disabled={disabled}>
      <span class="device-name">{missing.name}</span>
      <span class="device-detail">{t.settings.deviceUnplugged}</span>
    </button>
  {/if}
  {#if !known}
    <span class="loading">{t.settings.devicesLoading}</span>
  {/if}
</div>

<style>
  /* Mêmes classes que les choix du cache (surchargées par les thèmes), en colonne. */
  .cache-sizes {
    display: inline-flex;
    flex-direction: column;
    align-items: stretch;
    gap: 4px;
    min-width: min(420px, 100%);
    max-width: 640px;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: var(--radius-card, 14px);
  }

  .cache-size {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 16px;
    padding: 8px 14px;
    border-radius: var(--radius-pill);
    font-size: 13px;
    text-align: left;
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

  .device-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .device-detail {
    flex-shrink: 0;
    max-width: 50%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    opacity: 0.75;
  }

  .missing .device-detail {
    font-weight: 600;
    opacity: 1;
  }

  .loading {
    padding: 8px 14px;
    font-size: 12px;
    color: var(--text-muted);
  }
</style>
