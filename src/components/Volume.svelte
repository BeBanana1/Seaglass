<script lang="ts">
  // Volume: icon (mute / unmute), 0–100 slider and wheel. The displayed level is
  // that of the backend; only the drag in progress is kept locally.
  import { commands } from "../lib/ipc";
  import { player, stepVolume } from "../lib/stores/player.svelte";
  import { t } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";
  import { ui } from "../lib/stores/ui.svelte";

  const v = $derived(player.state.volume);
  /** Value being dragged, so that the slider does not jump between two states. */
  let dragging = $state<number | null>(null);
  const shown = $derived(dragging ?? (v.muted ? 0 : v.level));
  const icon = $derived(shown === 0 ? "volumeMute" : shown < 50 ? "volumeLow" : "volume");
  // Silent (muted or at 0): the icon restores the sound; the backend raises a level from 0.
  const label = $derived(shown === 0 ? t.player.unmute : t.player.mute);
  // Easter egg (#399): at maximum, the label goes up to 11. Accessibility keeps 100%.
  const levelText = $derived(
    shown === 100 && ui.settings.easterEggs ? "11" : t.format.percent(shown),
  );

  let root: HTMLDivElement;

  // `wheel` must be non-passive to prevent the page from scrolling.
  $effect(() => {
    const onWheel = (e: WheelEvent) => {
      if (e.deltaY === 0) return;
      e.preventDefault();
      stepVolume(e.deltaY < 0 ? 1 : -1);
    };
    root.addEventListener("wheel", onWheel, { passive: false });
    return () => root.removeEventListener("wheel", onWheel);
  });

  function onInput(value: number) {
    dragging = value;
    void commands.setVolume(value);
  }
</script>

<div class="volume" bind:this={root} title={t.player.volumeHint(levelText)}>
  <button
    class="mute"
    class:off={shown === 0}
    onclick={() => commands.setMuted(shown > 0)}
    aria-label={label}
    title={label}
  >
    <Icon name={icon} size={18} />
  </button>
  <div class="track" style:--ratio={shown / 100}>
    <div class="fill"></div>
    <input
      type="range"
      min="0"
      max="100"
      step="1"
      value={shown}
      aria-label={t.player.volume}
      aria-valuetext={t.format.percent(shown)}
      oninput={(e) => onInput(e.currentTarget.valueAsNumber)}
      onchange={() => (dragging = null)}
    />
  </div>
</div>

<style>
  .volume {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .mute {
    display: grid;
    place-items: center;
    padding: 4px;
    color: var(--text-muted);
    transition: color var(--motion-fast);
  }

  .mute:hover {
    color: var(--text);
  }

  .track {
    position: relative;
    width: 92px;
    height: 4px;
    border-radius: 2px;
    background: var(--border);
    box-shadow: var(--shadow-inset);
  }

  .fill {
    position: absolute;
    inset: 0 auto 0 0;
    width: calc(var(--ratio) * 100%);
    border-radius: inherit;
    background: var(--text-muted);
    transition: background var(--motion-fast);
  }

  .volume:hover .fill,
  .volume:focus-within .fill {
    background: var(--accent);
  }

  input {
    position: absolute;
    inset: -8px 0;
    width: 100%;
    margin: 0;
    appearance: none;
    background: transparent;
    cursor: pointer;
  }

  input::-webkit-slider-thumb {
    appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--text);
    opacity: 0;
    transition: opacity var(--motion-fast);
  }

  .volume:hover input::-webkit-slider-thumb,
  input:focus-visible::-webkit-slider-thumb {
    opacity: 1;
  }

  :global([data-theme="studio"]) .track,
  :global([data-theme="studio"]) .fill {
    border-radius: 0;
  }

  /* Studio's bar also shows the full audio chain: shorter slider. */
  :global([data-theme="studio"]) .track {
    width: 72px;
  }

  :global([data-theme="studio"]) input::-webkit-slider-thumb {
    width: 6px;
    border-radius: 0;
  }

  :global([data-theme="atelier"]) .mute {
    color: var(--text);
  }

  :global([data-theme="atelier"]) .track {
    height: 2px;
  }

  :global([data-theme="atelier"]) .fill,
  :global([data-theme="atelier"]) .volume:hover .fill {
    background: var(--text);
  }
</style>
